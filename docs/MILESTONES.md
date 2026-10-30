# Milestones and validation status

## Current branch — 0.56.14

0.56.14 adds bounded adaptive ruler labels, clip posters/audio labels and before/after reordering with no-op history protection. The shared project/renderer schema is unchanged. Real filmstrips, drag trim handles and photos remain pending.

0.56.13 establishes the desktop editor frame: searchable thumbnail media, large preview, clip inspector and persistent timeline. Shared session/history and output ownership are retained. Panel toggles preserve drafts; invalid hidden edits block output. Browser checks cover light/dark, normal/wide/minimum windows, preview seeking and shared commands. Timeline filmstrips/trim handles, mixed photo clips, audio waveforms and social tools remain pending. See [EDITOR_LAYOUT.md](EDITOR_LAYOUT.md). Local validation passed: 131 frontend tests, TypeScript/production build, native formatting/clippy and 140 Rust tests (6 ignored acceptance modes). The existing bundle-size warning remains; pushed-commit CI and installed visual acceptance are separate checks.

0.56.12 restores compact navigation, green section labels, saved sidebar collapse/expand and a shorter File Details toolbar. Map resizing and startup teardown are covered. Browser fixtures verified fixed row spacing at different window heights, light/dark icon rails, keyboard toggling, section labels and half-second video playback. Local checks passed: 127 frontend tests, TypeScript/production build, native formatting/clippy and 140 Rust tests (6 ignored acceptance modes). Existing bundle-size warnings remain. Pushed-commit CI and installed GUI visual acceptance are separate checks.

Phase E adds one ranked Library search over metadata and optional cached OCR/People/image-content evidence. Results explain why they matched, honor workspace/library filters, and retain metadata matches when a provider fails. Explicit Search/Enter adds meaning inference; typing does not repeatedly run the model. Workspace/current-index scope is applied before provider caps. See [UNIFIED_SEARCH.md](UNIFIED_SEARCH.md). Phase F must still implement video representative-frame indexing and matching timestamps.

0.56.11 local checks: 123 frontend tests, TypeScript/build, native formatting/lint and 140 Rust tests passed (6 ignored acceptance modes). Edge fixtures verified ranked search/reasons, source failures, facets and light/dark/minimum-window layout. Existing bundle-size warnings remain. The exact pushed GitHub Actions result and installed WebView2/hardware acceptance must be verified separately.

0.56.10 adds a small accessibility and destructive-action semantics pass on top of the 0.56.9 design-system cleanup. Original media behavior is unchanged.

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

0.56.9 retains the 0.56.8 continuous assembled preview/export and adds a presentation-only design-system completion pass across page headers and timeline controls. No project schema, renderer, indexing, or native media behavior changes.

0.56.8 adds continuous assembled preview/export, linked original audio and independent resolution presets to the shared timeline. One source decoder runs at a time. Output is validated and decoded before no-overwrite publication; originals are reverified. Short tests cover mixed clips, sound/silence/gain/offsets, repeat/trim/order, 4K and canvas shapes, preview agreement and failure safety. 111 frontend and 131 Rust tests passed (6 ignored acceptance modes), with formatting and warnings-as-errors native lint. Installed GUI acceptance remains open. See [rendering architecture](VIDEO_SEQUENCE_RENDERING.md).


0.56.7 moves project ownership to saved workspaces, supports sources from multiple registered folders, preserves older drafts through one-time transactional copies, and exposes bounded history/recovery. Projects remain editable when source folders are unavailable. Source bindings stay local; imported references and relinks are fingerprint-checked within registered workspace folders. One-video and multi-video editing now share the timeline/session/inspector/output controls; the old single-video controller is removed. One-clip rendering/export retains the tested pipeline through a saved-revision adapter. The assembled renderer remains next.

0.56.6 adds pure, validated timeline commands and an accessible timeline surface: trim, split, reorder, ripple deletion, repeat instances, durations/zoom, undo/redo, and revision-safe autosave. The Library opens the editor; the workspace selector remains at top left. Empty timelines preserve the media bin. This does not complete the assembled playback/export or shared editor/ownership migrations. See [DESIGN_SYSTEM_AUDIT.md](DESIGN_SYSTEM_AUDIT.md) and [DEVELOPMENT_PHASES.md](DEVELOPMENT_PHASES.md).

0.56.4 begins the feedback-driven UI cleanup alongside Phase 7: navigation/folder management, viewer navigation, video transport/trim wording, map wrapping, and a documented roadmap. These presentation changes do not replace the Phase 7 timeline work below.

0.56.3 delivers the project/media-bin foundation: bounded schema-v2 multi-asset documents, explicit v1 project-file migration into new drafts, multi-file import and content deduplication, selected-source previews, revision-checked saving/reopening, portable copies, and per-asset relinking. Phase 6 editing/export remains unchanged. Sources currently belong to one selected library folder and its subfolders. Timeline assembly and combined export remain next, not completed.

Multiple assets on one sequential timeline, split/trim/reorder/ripple-delete, linked audio, mixed-media normalization, save/relink/recovery, and continuous export. Start with hard cuts. See [VIDEO_ROADMAP.md](VIDEO_ROADMAP.md).

## Separately scoped future work

Reviewed People correction/merge/split tools, tag removal/rename and undo, cache management, and capability-tested Immich integration. Immich connectivity is not currently implemented.
