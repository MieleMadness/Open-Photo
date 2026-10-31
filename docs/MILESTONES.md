# Milestones and validation status

## Current source update — 0.56.32

0.56.32 fixes the scroll reset race and makes the virtual-row test frame-aware.

0.56.31 coalesces Library virtualization scroll updates to animation frames.

0.56.30 adds shared grid/list viewport-bound calculations and tests for large result sets.

0.56.29 virtualizes Library list rows with viewport-aware overscan and stable 64px geometry.

0.56.28 adds bounded, cache-aware Library thumbnail requests and skips stale queued thumbnails when users scroll quickly.

0.56.27 begins Phase G with viewport-aware Library grid virtualization and retained overscan; large-library performance acceptance remains pending.

0.56.26 adds bounded, debounced timeline frame scheduling and skips obsolete queued requests after zoom/resize or unmount. 0.56.25 added adaptive sample density, and 0.56.24 added timestamp-specific sampled frames. Windows installer acceptance is still required.

Requested groups 1/2/3/4/6 are implemented: native automatic AI queue/dashboard/exclusions, expanded zoomable details and simplified Live playback, fullscreen library filmstrip, signed Preview channel code/workflow, and bounded video semantic indexing with ranked timestamps. Source media stay read-only; folder Sync stays manual. See [handoff](LOCAL_HANDOFF_0_56_23.md), [AI architecture](AUTOMATIC_AI_INDEXING.md) and [viewer controls](MEDIA_VIEWER.md).

Local verification: 170 frontend tests, TypeScript and production build passed; native formatting, warnings-as-errors clippy and 161 Rust tests passed (7 acceptance modes ignored in the normal suite). The new ignored CPU-model video acceptance was also explicitly run and passed with a 0.6-second clip, cancellation/resume and unchanged-source checks. Edge fixtures verified both themes, normal/minimum-window sizing, wheel zoom/reset, fullscreen/Live controls and the AI dashboard. Existing bundle-size warnings remain. GitHub Actions for this source update and installed-app acceptance are unverified; the user will check manually. No installer/release publication or signed upgrade was performed.

Last published development baseline: 0.56.22. Preview signing/bootstrap and a real two-version upgrade remain deployment gates. Vault, large-library acceleration, map expansion and remaining text/music/transitions remain separate phases.


## Previous checkpoint — 0.56.22
0.56.22 completes the focused Library/search update: compact controls, Edits navigation, per-workspace folder/collection List/Tiles, manual Sync and offline saved-index restoration, automatic debounced semantic search and stable media labels. Cleanup/manual AI tools remain available outside Library. Automatic AI indexing/counters/exclusions, viewer refinements, encrypted vault and signed Preview channel remain planned; see AI_VIEWER_VAULT_ROADMAP.md. Local validation: 159 frontend tests, TypeScript/production build, native formatting/clippy and 146 Rust tests passed (6 explicitly ignored acceptance modes). Edge fixtures verified automatic semantic search, media labels, navigation, view modes and light/dark/minimum-window layout. Existing bundle-size warnings remain. Pushed-commit CI and installed GUI acceptance must be verified separately.

0.56.21 integrates typed still-photo clips into the shared editor and continuous renderer, with schema migration and normal first-run playback volume. JPEG/PNG/BMP/TIFF import, duration, source inspection, shared adjustments, undo/autosave/history/relink and photo-only or mixed output are covered with short fixtures. The new File Details location map is retained and regression-tested. Installed Windows acceptance, video semantic indexing and remaining social/music tools are pending. Local validation: 155 frontend tests, TypeScript/production build, native formatting/clippy and 145 Rust tests passed (6 explicitly ignored acceptance modes). Browser fixtures verified original-photo display, duration/undo, continuous playback and light/dark/minimum-window layout. Existing bundle-size warnings remain; exact pushed-commit CI must be verified separately.

0.56.20 aligns the remaining frontend trim tests with the timeline-only workflow; runtime behavior is unchanged.

0.56.19 adds global remembered video volume, lazy muted Library hover previews for videos and Live Photos, and five persisted accent palettes.

0.56.18 moves trimming fully to timeline handles, fixes preview/timeline divider snap-back, adds direct timeline zoom controls, and introduces filmstrip-style clip previews using the bounded poster cache.


0.56.17 separates Clip and Canvas & export inspector tasks, with direct Canvas access, keyboard navigation and draft-preserving panels. Invalid clip drafts are revealed before output. Filmstrips and photo clips remain pending.

0.56.16 adds a saved, keyboard-accessible preview/timeline divider. Resizing changes only app layout, with viewport/notice-aware bounds and cancellable drafts. Filmstrips and photo clips remain pending.

0.56.15 adds pointer/keyboard trim handles with local gesture drafts, stale-gesture protection and one-step undo/autosave. Browser fixtures verify drag, cancellation, keyboard nudging and minimum-window layout. Filmstrips, adjustable pane splits and photo clips remain pending.

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
