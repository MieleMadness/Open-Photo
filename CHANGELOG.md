# 0.54.9 — Previous-save recovery for video projects

- Anchor reference-frame inspection between earlier random-access points, with no initial seek when there is insufficient preroll information. Add keyframe-boundary regressions and actionable empty-window diagnostics.
- Freeze completed frame-inspection windows so buffered/partial output during probe termination cannot alter reference counts. Add cancellation-tail regression coverage.
- Update checkout, Node setup, and artifact upload actions to verified Node 24 releases; keep the app build Node version pinned.

- Browse up to 20 prior automatic saves with available save number/time, title, trim range, and canvas dimensions. Open a selected version as a separate recovered draft and return to the previous draft.
- Preserve the current draft and its history. Carry forward the current verified source identity and relink path while recovering historical edit settings. Missing sources remain explicitly unavailable until relinked.
- Check library/project ownership, snapshot identity, and the current save revision inside a write transaction. Reject stale, pruned, oversized, or incompatible snapshots instead of replacing current work.
- Migrate existing history storage atomically and idempotently. Preserve legacy saves with unknown number/time rather than inventing metadata; record those fields for subsequent saves. Keep recovered Unicode titles within editor limits.
- Add migration, retention/reopen, exact edit recovery, relink, conflict/ownership, source protection, UI recovery/return, empty-history, and failure coverage. Preserve editing during pending previews; only recovery locks edit controls.
- Installed-app playback, complete long-export/perceptual synchronization, drive-failure, and whole-app performance acceptance remain open. Phase 7 remains planned.

# 0.54.8 — Long-video navigation and seek acceptance

- Add Go to time for seconds, minutes:seconds, and hours:minutes:seconds with millisecond precision. Validate malformed and out-of-range entries before seeking; keep browser seeking explicitly approximate.
- Add a synthetic hour-scale source test that exports short windows near the beginning, middle, and end, checks decoded frame counts/timestamps against the source, validates independent audio/video spans, fully decodes outputs, and verifies the source hash.
- Extend opt-in real-footage acceptance with -LongRecording, saving three separate test edits, portable projects, and a local report. Stream decoded frame timestamps until the presentation range is complete, preserving buffered B-frames, with timeout and frame/line limits.
- Keep complete hour-long export, perceptual audio/video synchronization, installed-app playback, drive failures, and whole-app performance as separate acceptance gates.

# 0.54.7 — UHD and source-resolution video export

- Add explicit UHD landscape (3840×2160), UHD portrait (2160×3840), and supported source-resolution canvas choices. Preserve ultra-wide source geometry, square-pixel aspect, and source rotation when choosing source resolution; retain imported canvases.
- Scale the target video bitrate from 12 Mbps at 1080p to 48 Mbps at UHD, bounded to that range. Use the same resolution policy for conservative destination-space estimates. Keep the 1080p export limit and smaller-preview policy intact.
- Add actual UHD landscape/portrait/1080p-limit export tests and editor/source-canvas coverage.
- Add opt-in real-footage export acceptance, with source hashes, full decode, publication, separate source-resolution/UHD test files, portable project copies, and local reports. A UHD canvas does not add detail to lower-resolution footage.
- Pin the workflow run name to the source release version so a stale PR title cannot mislabel a build.
- Installed-app/real-media playback, hour-scale synchronization, actual drive failures, and whole-app performance remain acceptance gates.

# 0.54.6 — Repeatable release video acceptance

- Add an opt-in optimized-build acceptance test using an eight-second 1080p/audio fixture, non-frame-grid trimming, crop, rotation, gain, and the real export/publication pipeline.
- Record export wall time, processing rate, stage start times, output properties, and the maximum individual native child peak working set. Memory instrumentation exists only in test builds; this does not measure whole-app memory.
- Save a machine-labeled environment report without personal library paths or host identifiers. Fresh output directories and report creation prevent reuse of stale results.
- Run the acceptance test in the Windows installer job and publish JSON reports alongside the installer. Keep real-media/GUI, hour-scale synchronization, actual drive failures, startup/seek performance, and whole-app memory as open Phase 6 gates.

# 0.54.5 — Phase 6 export hardening

- Validate video and audio spans independently, so longer audio cannot hide truncated video. Preserve legitimately unequal source tracks and omit audio when the kept range has none.
- Inspect packet timing with bounded streaming reads when source containers omit stream durations. Require timing metadata on generated MP4 tracks and cap video timing tolerance at 100 ms.
- Prevent delayed or out-of-order export-status responses from clearing a newly started job or regressing completed progress.
- Add the directory separator required by Windows free-space checks for UNC shares.
- Add timing and status-race regression coverage. Installed-app GUI, real-footage/hardware, and measured performance/memory acceptance remain incomplete.

# 0.54.4 — Phase 6.4: Validated video export

- Export saved single-video edits to a new SDR H.264/AAC MP4 using the same trim/crop/rotation/flip/audio/canvas graph as edited previews. Offer the project canvas or an orientation-aware 1080p limit.
- Export an immutable saved snapshot, with explicit first-audio/stereo policy and confirmation for multiple source tracks. Drop subtitles, chapters, and source metadata intentionally; reject detected HDR until tone mapping is tested.
- Add an app-wide progress/elapsed/cancellation panel. Exports survive editor closure, and normal app closure waits for exports to finish or be cancelled. Serialize native processing with preview jobs.
- Check destination space, stage beside the destination, validate streams/timing/geometry/size, fully decode output, reverify source identity, then publish without overwrite. Cancel/failure cleans only the job's temporary file; racing destination files survive.
- Fix timestamp quantization in compatible previews and edited render/export by carrying source encoder time bases and a finer movie clock. Add jittered-VFR and non-frame-grid trim coverage.
- Add decoded-output, preview/export pixel/timestamp/audio comparisons, a 90-second VFR/audio fixture, portrait 1080p, cancellation/commit race, corrupt-output, changed-source, disk-space, overwrite, and UI lifecycle tests.
- Real-media/hardware/performance acceptance remains required before Phase 6 can be considered complete. HDR tone mapping and waveform UI remain deferred.

# 0.54.3 — Phase 6.3: Saved single-video edits

- Add versioned projects with stable asset/clip IDs, relative source paths, full SHA-256 fingerprints, rational source-time trim ranges, transform/audio settings, and output canvas settings.
- Save drafts in SQLite with revision checks and bounded previous-document history. Serialize/coalesce autosaves, retain pending saves when switching media, block window closure while saving, and surface failure/retry/recovery-copy options.
- Add recent drafts, session undo/redo/reset, non-overwriting portable project copies, strict project-file validation, and fingerprint-verified relinking.
- Persist trim, crop (free and common aspect presets), quarter-turn rotation, flips, mute/gain, and fit/fill canvas choices. Generate cancellable edited previews through a shared validated processing graph, rather than CSS transforms.
- Check source fingerprints before/after rendering. Add decoded-pixel, audio-gain/mute, VFR-trim, persistence/conflict/relink, copy protection, queue recovery, and close-guard tests.
- Correct video details sizing so long player/editor controls scroll instead of being clipped inside a photo preview box.
- Export, HDR tone mapping, waveform UI, and real-media/hardware acceptance remain incomplete Phase 6 work.

# 0.54.2 — Phase 6.2: Playback and compatible previews

- Add play/pause, a scrubber, time display, keyboard seeking, clickable filmstrip frames, and temporary in/out markers with marked-range playback.
- Preserve preview marks as integer source ticks; switching between original and compatible playback retains the selection and playhead. These markers do not yet save or trim a video.
- Add serialized, cancellable background jobs for filmstrips and compatible SDR H.264/AAC previews. Apply rotation and square-pixel scaling while preserving variable frame timing and relative audio offsets.
- Keep original media read-only and stream video bytes. Bound derivative size, duration, dimensions, runtime, and thumbnail output; surface errors while retaining original/Windows playback options.
- Give native video processes Windows job ownership so app exit terminates native work. Clean session-owned derivatives on release and retry abandoned-cache cleanup after a day.
- Prevent seeking shortcuts from navigating to another library item; ignore stale jobs and cancel work on player closure/source changes.
- Add real-media timing/rotation/audio/large-file/source-preservation tests, process ownership and cancellation cleanup tests, and player/marker lifecycle coverage.
- HDR proxy conversion, saved editing projects, undo/redo, and export remain incomplete Phase 6 work.

# 0.54.1 — Phase 6.1: Local video inspection

- Bundle a fixed, checksum-verified LGPL FFmpeg/ffprobe engine and its shared libraries/licenses in the Windows installer.
- Show video duration, dimensions, codec, exact frame rate, rotation, HDR/color information, audio tracks, and inspection warnings alongside existing playback.
- Inspect canonical local media paths in bounded background jobs, with timeout, superseded-job cancellation, bounded diagnostics, and network/playlist input rejection.
- Expose bundled decoder/encoder inventories for subsequent editing milestones.
- Add real-engine MP4/MOV, Unicode-path, audio/silent, rotation, HDR, variable-timing, corrupt-input, and process-lifecycle tests.
- Add a reproducible local check script and persist the user's development instructions in AGENTS.md.
- Video editing controls, proxies, project persistence, and export remain incomplete Phase 6 work.

# 0.54.0 — Library integrity and editing foundations

- Fixed the Tags initialization crash, complete metadata batching, active-filter invalidation, and partial tag-write reporting.
- XMP keyword merges preserve existing XML properties and use namespace-aware parsing. Conflicting external edits are detected before replacement; Windows replacement is atomic with a recovery copy.
- Incomplete scans retain previous index snapshots, with visible read errors.
- Shared image decode normalizes EXIF orientation and applies explicit allocation/pixel limits. Editor jobs run off the UI thread, serialize allocations, and drop superseded queued previews.
- Editor previews use export-scale blur/sharpen semantics and a bounded normalized-source cache. Completed exports publish atomically without overwriting another file.
- Inline video uses scoped range-aware local streaming rather than whole-file base64; decoder-dependent fallback remains available.
- Organizational data migrates into local SQLite with previous-value history; startup load failures do not replace saved data. Settings can export a separate JSON metadata backup.
- People IDs and names persist independently of filenames; named identities are never automatically merged with new suggestions. Existing compatible legacy names migrate when their original group is available.
- Fixed save-dialog permissions and incremental Live Photo metadata loss.
- Added dependency lockfiles, pinned toolchains, frontend regression tests, independent frontend/Rust checks, and gated installer packaging.
- Added an authoritative Phase 6/7 roadmap. Video editing itself remains planned.

# Changelog

This file records the major Open-Photo feature milestones, important add-ons, safety changes, and notable build repairs.

Open-Photo is still in **0.x active development**. During this phase:

- a green predecessor advances to the next **minor** version for feature work;
- a red build is repaired with a **patch** version;
- patch releases often contain formatting, lint, regression, or compatibility fixes rather than new user-facing features.

The detailed Git commit history remains the source of truth for every individual change.

## 0.53.1 — 2026-10-27

### Fixed
- Applied the exact Rust formatter change required by CI for the managed tag read regression test.
- No hierarchical tag behavior, filtering, XMP ownership rules, or safety behavior changed.

## 0.53.0 — 2026-10-27

### Added
- Added read-back of keywords from Open-Photo-managed XMP sidecars.
- Replaced the Tags placeholder with a real hierarchical Tags browser.
- Tags use `/` as the hierarchy separator, such as `People/Family/Marc`.
- Parent branches are generated automatically and show unique tagged-media counts.
- Selecting a branch opens the Library filtered to that branch and all descendant tags.
- Added a visible active-tag filter chip and a Refresh tags action.
- Added regression coverage proving tag reads ignore foreign/unknown XMP sidecars.

### Changed
- Major date/smart/collection filters clear an active tag filter so organization filters do not stack invisibly.

### Safety
- Tag indexing is read-only and only parses XMP sidecars carrying the Open-Photo ownership marker.
- Foreign/unknown sidecars remain ignored and untouched.
- Managed sidecar reads are capped at 256 KB and 5,000 media paths per request.

## 0.52.1 — 2026-10-27

### Fixed
- Applied the exact Rust formatter changes required by CI for the managed XMP merge implementation.
- No XMP behavior, ownership rules, limits, or source-media safety behavior changed.

## 0.52.0 — 2026-10-27

### Added
- Added safe keyword merging for XMP sidecars created and owned by Open-Photo.
- New Open-Photo sidecars are marked with `xmp:CreatorTool="Open-Photo"`.
- Existing Open-Photo keywords are preserved while new tags are merged case-insensitively.
- The tagging UI now reports created, updated and skipped sidecars separately.
- Added regression coverage proving foreign XMP files remain untouched and managed sidecars merge tags without modifying source media.

### Changed
- Foreign or unknown XMP sidecars are still skipped rather than overwritten.
- Managed sidecar replacement now uses a temporary file plus rollback backup instead of truncating an existing XMP file in place.

### Safety
- Original photos and videos are never edited by the tagging workflow.
- XMP sidecars larger than 256 KB or not carrying the Open-Photo ownership marker are skipped.
- Managed sidecars are limited to 200 keywords.

## 0.51.0 — 2026-10-27

### Added
- Added local embedded-preview support for Nikon NEF and Sony ARW RAW files.
- The Library grid, list thumbnails and full viewer can now use the largest embedded JPEG preview available in supported NEF/ARW files.
- RAW preview orientation is applied from the camera metadata before the image is shown.

### Changed
- RAW files can use a larger bounded source-file limit for preview extraction because the preview reader seeks to embedded JPEG data instead of decoding the RAW sensor payload.
- Full RAW sensor-data development/editing remains intentionally out of scope for this milestone.

### Safety
- RAW preview extraction is read-only.
- The preview library applies bounded directory/preview limits and does not decode raw sensor data.
- Open-Photo caps supported RAW source files at 250 MB for this preview path.

## 0.50.0 — 2026-10-27

### Added
- Added Apple Live Photo content-identifier extraction for supported still images and MOV/MP4/M4V motion clips.
- Added persistent `live_asset_id` indexing for full scans and incremental updates.
- Live Photo pairing now prefers exact shared Apple asset IDs.
- The Library reports metadata-matched versus filename-fallback pair counts.
- Added regression coverage for Apple MakerNote tag 0x0011 parsing.

### Changed
- Same-folder + same-filename-stem pairing remains as a conservative fallback only when Apple asset metadata is absent or stripped by an exporter.

### Safety
- Live Photo identifier extraction reads metadata only; it never modifies source stills or movies.
- Movie identifier parsing skips large media payloads and bounds the in-memory `moov` metadata read to 16 MB.
- Ambiguous asset-ID groups are not paired automatically.

## 0.49.0 — 2026-10-27

### Added
- Added pure-Rust metadata fallback support for HEIC/HEIF, AVIF, Canon CR3, Fujifilm RAF and Phase One IIQ containers.
- HEIC/HEIF capture dates can now participate in Timeline, Memories and date sorting when embedded EXIF capture metadata is present.
- Added CR3, RAF and IIQ to supported library indexing for metadata-focused workflows.
- Added regression coverage for timezone-less and timezone-aware extended metadata date normalization.

### Changed
- Existing JPEG/TIFF EXIF parsing and the verified MOV/MP4/M4V QuickTime parser remain unchanged.
- RAW metadata indexing is now broader, but full RAW pixel previewing is still a separate unfinished feature.

### Safety
- Extended metadata parsing is read-only and does not decode or rewrite source image pixels.
- Invalid or missing metadata safely falls back to filesystem modified time.

## 0.48.0 — 2026-10-27

### Added
- Added capture-date coverage counts to each year in the Memories **All years** browser.
- Each year now reports how many items use **EXIF metadata**, **QuickTime metadata**, or **file modified fallback**.

### Changed
- Memories makes date-source quality visible before users rely on year-based organization, helping identify libraries that still depend heavily on filesystem timestamps.

### Safety
- Coverage counts are derived from already-indexed metadata and do not trigger additional source-file reads.
- No source media or metadata is moved, copied, renamed, or modified.

## 0.47.0 — 2026-10-27

### Added
- Added an **All years** browser to Memories.
- Every capture year in the current workspace is listed with its indexed media count.
- Any year can be opened directly in the Library using the existing Memory year filter, even when there is no On This Day match.

### Changed
- Memories now remains useful on dates with zero On This Day results by providing full year-based navigation from the same view.

### Safety
- Year counts are derived from the already-loaded local index and do not trigger additional source-file reads.
- No source media or metadata is moved, copied, renamed, or modified.

## 0.46.0 — 2026-10-27

### Added
- Added **Browse all media from YEAR** to each Memories year group.
- Added a dedicated Memory year filter to the Library with a visible **Show all years** control.
- Year filtering uses the same capture-date preference as Memories and Timeline.

### Changed
- Memory year, Timeline month, smart collection, custom smart rule, and virtual collection filters are kept mutually exclusive to avoid hidden filter stacking.

### Safety
- Year browsing is a presentation-only filter over the existing local index.
- No source media or metadata is moved, copied, renamed, or modified.

## 0.45.0 — 2026-10-27

### Added
- Added a dedicated **Memories** view to the main navigation.
- On This Day matches are grouped by capture year and labeled by how many years ago they were captured.
- Individual memories can be opened directly from the Memories view.
- Added one-click actions for **Open all On This Day** and **This month in past years**.

### Changed
- Memories now uses the same verified EXIF / QuickTime / modified-date fallback rules as Timeline and smart collections instead of being discoverable only as a smart filter.

### Safety
- Memories is a read-only presentation layer over the existing local index.
- No source media or metadata is moved, copied, renamed, or modified.

## 0.44.0 — 2026-10-27

### Added
- Timeline month entries now open the Library filtered to that exact year/month.
- The Library shows the active Timeline month with a **Show all months** control.
- Added a stable internal `YYYY-MM` month key so localized display labels do not drive filtering logic.

### Changed
- Timeline month filtering uses the same EXIF / QuickTime / modified-date fallback logic as sorting and smart collections.
- Timeline month filtering is cleared when a smart collection, custom smart rule, or virtual collection is selected to avoid hidden filter stacking.

### Safety
- Timeline filtering is presentation-only and does not move, rename, copy, or modify source media.

## 0.43.0 — 2026-10-27

### Added
- Added explicit date provenance to the media details panel.
- The active date source is labeled as **EXIF metadata**, **QuickTime metadata**, or **File modified fallback**.
- When embedded capture metadata exists, the details panel also shows both the embedded capture value and the filesystem modified value.

### Changed
- Timeline help text now reflects the verified EXIF + QuickTime capture-date behavior instead of describing QuickTime dates as planned.

### Safety
- Provenance is derived from already-indexed metadata and does not trigger additional file reads.
- No source media or metadata is modified.

## 0.42.1 — 2026-10-27

### Fixed
- Applied the exact Rust formatter output required by CI for the QuickTime capture-date parser and tests.
- No QuickTime parsing, timestamp conversion, timeline, sorting, or smart-filter behavior was changed.

## 0.42.0 — 2026-10-27

### Added
- Added read-only QuickTime movie-header capture-date extraction for MOV, MP4 and M4V media.
- The parser walks ISO base-media/QuickTime box headers to find `moov → mvhd` without decoding video frames or loading the full file into memory.
- Supports both version 0 (32-bit) and version 1 (64-bit) `mvhd` creation timestamps.
- Added regression coverage for the QuickTime 1904 epoch and a minimal movie-header container.

### Changed
- Timeline, newest/oldest sorting and date-based smart collections can now use QuickTime video capture time before falling back to filesystem modified time.
- QuickTime timestamps are stored as UTC ISO strings with a `Z` suffix; EXIF timestamps remain timezone-less camera-local values.

### Safety
- Invalid box sizes, unsupported movie-header versions and corrupt containers return no capture date and safely fall back to modified time.
- Video files are opened read-only and never rewritten.
- Capture-date parsing seeks over large media payloads instead of buffering them into memory.

## 0.41.4 — 2026-10-27

### Fixed
- Added explicit calendar/time range checks after EXIF parsing so impossible values such as month 13 or hour 25 are rejected.
- This fixes the capture-date regression test while keeping the EXIF library parser for structural parsing.
- No source media is modified.

## 0.41.3 — 2026-10-27

### CI
- Pinned the Windows CI job to the supported `windows-2022` hosted runner pool after repeated `windows-latest` jobs failed before step execution and produced no logs.
- Application and capture-date behavior are unchanged.

## 0.41.2 — 2026-10-27

### Fixed
- Replaced manual EXIF timestamp splitting with the kamadak-exif `DateTime::from_ascii` parser already used by the project.
- Capture timestamps are now validated by the EXIF library before being stored.
- No source files are modified.

## 0.41.1 — 2026-10-27

### Fixed
- Applied the exact Rust formatter output required by CI for the capture-date foundation.
- No EXIF parsing, SQLite indexing, timeline, sorting, or smart-filter behavior was changed.

## 0.41.0 — 2026-10-27

### Added
- Added EXIF capture-date extraction during library scans for supported JPEG/TIFF-family still images.
- Capture date preference order is **DateTimeOriginal → DateTimeDigitized → DateTime**.
- Added persistent `capture_local` storage to the SQLite media index with an in-place schema migration.
- Incremental file updates now refresh capture-date metadata as well.

### Changed
- Timeline now prefers capture date and falls back to file modified date.
- Newest/oldest sorting and date-based smart collections now use the same capture-date-first behavior.
- File details now show both **Captured** and **Modified** dates.

### Safety
- Camera timestamps without a trustworthy timezone are stored as local date/time strings; Open-Photo does not invent a timezone.
- Capture-date extraction is read-only and does not modify media.
- QuickTime/iPhone video capture dates are not yet claimed as supported.

## 0.40.0 — 2026-10-27

### Added
- Added centered crop presets for **Original / Free**, **1:1**, **4:3** and **16:9**.
- Crop presets use the decoded source dimensions returned by the existing preview request.
- Preset calculations account for the current 90° rotation before determining the centered crop.

### Changed
- Crop sliders now use 0.1% precision so preset-generated crop values can be adjusted without being rounded to whole percentages.

### Safety
- Crop presets still use the existing 0–45% per-edge bounds and preserve the existing minimum-image-area validation.
- No additional source-file read or write is performed for crop preset calculation.
- Original media remains read-only.

## 0.39.0 — 2026-10-27

### Changed
- Centralized the frontend editor source-format list so preview visibility and export availability use the same contract.
- Added strict whole-number resize parsing before preview and export requests.
- Animated GIF is no longer offered to the Phase 4 editor because the current image pipeline would otherwise render only a decoded still frame.

### Safety
- Invalid, fractional, zero, negative and over-20,000 resize values are rejected before they reach the Rust edit pipeline.
- Backend preview and export now both validate the same supported source extensions.
- Original media remains read-only.

## 0.38.1 — 2026-10-27

### Fixed
- Applied the exact Rust formatter output required by CI for the bounded-preview geometry helpers and regression tests.
- No preview, geometry, sizing, or export behavior was changed.

## 0.38.0 — 2026-10-27

### Changed
- Refactored editor geometry so rotation, crop and resize dimensions are calculated through shared backend helpers.
- Live preview now uses a bounded preview-size resize instead of allocating the full requested export dimensions first.
- Final export width and height are still calculated from the exact requested edit geometry and reported unchanged.

### Safety
- Large but valid export targets no longer require equally large live-preview resize buffers.
- The 100-megapixel final-output limit remains enforced before preview or export work proceeds.
- Original media remains read-only.

## 0.37.1 — 2026-10-27

### Fixed
- Applied the exact Rust formatter output required by CI for the 100-megapixel regression test.
- No output-size validation, preview, or export behavior was changed.

## 0.37.0 — 2026-10-26

### Added
- Added backend-reported final edited output dimensions to the live preview.
- Added visible width × height and megapixel count before export.
- Added a hard **100 megapixel** edited-output limit.

### Safety
- The pixel cap is enforced before resize allocation, reducing the risk of accidental extremely large image allocations.
- Preview and export share the same output-dimension validation.
- Original media remains read-only.

## 0.36.1 — 2026-10-26

### Fixed
- Applied the exact Rust formatter output required by CI for the export-date preservation regression test.
- No timestamp-preservation, preview, or export behavior was changed.

## 0.36.0 — 2026-10-26

### Added
- Added **Preserve source modified date** for edited exports, enabled by default.
- Added an opt-out so users can use the export time instead.
- Export confirmation now reports when the source date was preserved.

### Fixed
- Removed duplicated editor reset state when switching photos.
- Reset Edits now restores all current editor/export controls to their defaults.

### Safety
- Timestamp preservation changes only the new edited copy.
- If the requested source timestamp cannot be applied, the new export is removed and the operation reports an error.
- Original media remains read-only.

## 0.35.1 — 2026-10-26

### Fixed
- Applied the exact Rust formatter output required by CI for the JPEG encoder import.
- No before/after comparison, JPEG quality, preview, or export behavior was changed.

## 0.35.0 — 2026-10-26

### Added
- Added **Show original / Show edits** comparison directly in the photo preview.
- Added explicit JPEG export quality from 40–100 with a default of 92.
- Added backend validation for JPEG quality.
- Added regression coverage for JPEG quality bounds.

### Fixed
- Reset Edits now clears grayscale, hue, blur, sharpen and sharpen-threshold controls too.
- Removed duplicated advanced-adjustment resets when switching photos.

### Safety
- Before/after comparison is read-only.
- JPEG quality only changes newly rendered JPEG copies; originals remain untouched.

## 0.34.1 — 2026-10-26

### Fixed
- Applied the exact Rust formatter output required by CI for the advanced-adjustment regression tests.
- No grayscale, hue, blur, sharpen, preview, or export behavior was changed.

## 0.34.0 — 2026-10-26

### Added
- Added grayscale editing with live preview.
- Added hue rotation from -180° to +180°.
- Added blur from 0.0 to 10.0.
- Added sharpening from 0.0 to 10.0 with a 0–100 threshold.
- Added backend validation and regression tests for the new adjustment limits.

### Safety
- All new adjustments use the existing shared preview/export pipeline.
- Original media remains read-only and export remains create-new only.

## 0.33.1 — 2026-10-26

### Fixed
- Applied the exact Rust formatter output required by CI for the new crop/resize regression tests.
- No Phase 4 crop, resize, preview, or export behavior was changed.

## 0.33.0 — 2026-10-26

### Added
- Added **live edit preview** generated by the same Rust pipeline used for final export.
- Added independent crop controls for left, right, top and bottom edges.
- Added resize by width, height, or exact dimensions.
- Added aspect-ratio-preserving resize when only one dimension is supplied.
- Added **Reset edits** for the complete edit state.
- Added regression tests for crop/resize dimensions and excessive-crop rejection.

### Safety
- Crop settings are bounded to 0–45% per edge and must leave at least 10% of the image.
- Resize dimensions are limited to 20,000 pixels per side.
- Export remains create-new only; originals and existing destination files are never overwritten.

## 0.32.2 — 2026-10-26

### Fixed
- Replaced the eight-argument edited-export command with a typed `EditOptions` request object to satisfy strict Clippy without suppressing lint rules.
- Updated the frontend export call to use the typed edit-options payload.
- No Phase 4 editing functionality was removed.

## 0.32.1 — 2026-10-26

### Fixed
- Applied the exact Rust formatter output required by CI for the new non-destructive edited-export backend.
- No Phase 4 editing functionality was removed or rolled back.

## 0.32.0 — 2026-10-26

### Added
- Started **Phase 4** with a non-destructive **Edit & Export** workflow.
- Added 90° rotation, horizontal/vertical flip, brightness and contrast adjustments.
- Added an explicit save dialog for exporting a new edited copy.
- Added backend export validation that prevents overwriting the original source or any existing destination.
- Added cleanup of partial exports if encoding or disk sync fails.
- Added regression tests for edit limits and quarter-turn dimensions.

### Safety
- Original media remains read-only.
- Edited output is always a new file chosen by the user.
- Existing files are never silently overwritten.

### Limitations
- First-pass editing supports common locally decodable still-image formats; HEIC/HEIF, RAW and video editing remain excluded.
- Export supports JPG/JPEG, PNG, WebP, BMP and TIFF.
- Metadata preservation is not yet guaranteed for rendered edited copies.

## 0.31.0 — 2026-10-26

### Added
- Added a backend **Phase 3 AI self-check** report.
- Added Settings UI to run the self-check and report Semantic Search, People and OCR bundle readiness.
- Added DirectML availability and CPU-fallback status to the self-check.
- Added tests that distinguish missing from incompatible model slots.

### Changed
- Photo-details semantic AI actions are now disabled until the verified OpenCLIP bundle is ready.
- Documentation now distinguishes **Phase 3 core implementation complete** from **real-world validation still required**.

### Phase 3 validation still required
- Representative real-library testing for semantic search, People and OCR.
- CPU-vs-DirectML comparison on real Windows hardware.
- People grouping threshold tuning and OCR post-processing refinement.
- Stronger pause/resume behavior if very long AI indexing jobs require persistent queues.

## 0.30.0 — 2026-10-26

### Added
- Added backend-provided compatibility metadata for every supported AI model slot.
- Added Settings readiness states for **Semantic Search**, **People**, and **OCR**.
- Added model-level **verified / missing / incompatible** status in the Local AI inventory.
- Added a regression test that every known AI slot has a valid pinned SHA-256 contract.

### Changed
- Removed the duplicated checksum table from the React frontend; Rust is now the single source of truth for supported AI assets.
- Model imports now use the backend-provided expected checksum.
- Semantic Search, People, and OCR actions are disabled until their complete verified model bundles are ready.

### Phase 3 status
- Core implementation now exists for semantic search, DirectML-first execution with CPU fallback, People/face grouping, and English-first OCR.
- These AI features remain experimental until representative real-library and real-hardware validation is complete.

## 0.29.2 — 2026-10-26

### Fixed
- Added the missing `serde_json` dependency required to persist OCR region metadata in local application data.
- No OCR functionality was removed or rolled back.

## 0.29.1 — 2026-10-26

### Fixed
- Applied the exact Rust formatter output required by CI for the new OCR pipeline and OCR model guards.
- No OCR functionality was removed or rolled back.

## 0.29.0 — 2026-10-26

### Added
- Added the first working **OCR** pipeline for local photo-text indexing and search.
- Pinned the English PP-OCRv3 mobile detector by SHA-256.
- Pinned the English PP-OCRv4 mobile recognizer by SHA-256.
- Added local detector preprocessing, probability-map region extraction and conservative text-region bounding boxes.
- Added local English CTC text recognition using the pinned PP-OCR character contract.
- Added persistent OCR text and region metadata in `ai-ocr.sqlite`.
- Added incremental OCR indexing that skips photos already current for the same source timestamp and model pair.
- Added a Library toolbar for **Index library text** and **Search photo text**.
- Reused the existing DirectML-first / CPU-fallback AI processing preference.

### Safety
- OCR never modifies original media.
- Extracted text remains local application data unless the user later chooses an explicit export/write workflow.

### Limitations
- The first OCR release is English-first.
- Text-region post-processing uses conservative rectangular connected components rather than full DB polygon reconstruction.
- Rotated, curved, handwritten and dense document text still need validation and refinement.

## 0.28.2 — 2026-10-26

### Fixed
- Reworked ArcFace landmark packing to satisfy the strict Clippy iterator rule.
- Switched People-group ordering to `sort_by_key(Reverse(...))` as required by the Windows lint gate.
- No People recognition functionality was removed or rolled back.

## 0.28.1 — 2026-10-26

### Fixed
- Applied the exact Rust formatter output required by the Windows CI gate for the new People pipeline and face-model import guard.
- No People functionality was removed or rolled back.

## 0.28.0 — 2026-10-26

### Added
- Added the first working **People** pipeline using pinned OpenCV Zoo models.
- Added exact YuNet 2026may face-detector enforcement (MIT weights, SHA-256 pinned).
- Added exact SFace 2021dec face-embedder enforcement (Apache-2.0 weights, SHA-256 pinned).
- Added local YuNet preprocessing, 12-head output decoding, confidence filtering and non-maximum suppression.
- Added five-landmark ArcFace alignment to 112×112 crops before SFace inference.
- Added normalized 128-value face embeddings and a separate local `ai-faces.sqlite` index.
- Added per-photo model provenance and source-modification tracking so current People results can be skipped safely.
- Added multi-photo People indexing with shared ONNX sessions per batch.
- Replaced the People placeholder with local suggested groups, index status and manual local naming.
- Added automatic use of Open-Photo's pinned checksums for known AI model imports.

### Safety
- People indexing never modifies original photos.
- Suggested identity groups are advisory and are not permanently auto-merged.
- Face embeddings remain local application data and are not exported by default.

### Notes
- People grouping is experimental and still needs representative real-library validation and threshold tuning.
- OCR remains unfinished.

## 0.27.2 — 2026-10-26

### Fixed
- Kept the DirectML-aware ONNX session builders mutable through model commit for both semantic image and semantic text inference.
- This resolves the Rust Clippy/compile error introduced by the 0.27.0 GPU-provider integration.

## 0.27.1 — 2026-10-26

### Fixed
- Applied the exact Rust formatter layout required by the Windows CI fail-fast gate for the new DirectML-aware semantic image/text commands.
- No Phase 3 functionality was removed or rolled back.

## 0.27.0 — 2026-10-26

### Added
- Added Windows **DirectML-first** ONNX execution for Automatic AI processing mode with safe CPU fallback.
- Added local AI runtime status reporting so Settings can show whether DirectML is available.
- Added visible semantic indexing progress in bounded 64-photo batches.
- Added safe cancellation between AI indexing batches; embeddings completed before cancellation remain stored and reusable.

### Changed
- Routed semantic image indexing and natural-language text inference through the selected AI processing preference.
- Updated the Settings copy so GPU behavior is explicit instead of being labeled as future support.
- Updated the in-app version display to 0.27.0.

### Phase 3 guardrails
- Face recognition and OCR model slots remain intentionally non-executable until Open-Photo pins supported model contracts (preprocessing, output shape, checksums, licensing and regression fixtures). Arbitrary ONNX files are not treated as compatible merely because they import successfully.
- DirectML still requires real-hardware validation against the CPU path before semantic search is considered production-ready.

## 0.26.2 — 2026-10-26

### Fixed
- Made the AI Library toolbar callbacks explicitly null-safe so TypeScript does not lose the surrounding `result` guard when those callbacks execute later.

## 0.26.1 — 2026-10-26

### Fixed
- Applied the exact Rust formatter output to the incremental semantic indexing path so the Phase 3 library-indexing milestone can reach compile, lint, regression-test and installer validation.

## 0.26.0 — 2026-10-26

### Added
- Added **Index library with AI** for the current workspace.
- Added multi-root AI indexing orchestration in the Library UI.
- Added multi-root natural-language semantic search that merges and ranks matches across workspace folders.
- Added explicit batch failure counts separate from safe already-current skips.

### Performance
- Semantic batch indexing now reuses one ONNX vision session per batch instead of loading the model once per photo.
- Heavy batch inference runs on Tauri's blocking worker pool rather than the main UI thread.
- Existing embeddings are skipped when the source modification time, semantic pipeline key and 512-value dimension are already current.
- Raised the backend batch limit to 5,000 photos; the UI chunks larger root groups.

### Notes
- Full cancellable queue/progress/pause/resume support is still planned.
- Semantic search remains experimental until the verified OpenCLIP bundle is exercised end-to-end on representative photo libraries.

## 0.25.3 — 2026-10-26

### Fixed
- Disabled the optional `tokenizers` `esaxx_fast` C++ acceleration feature on Windows builds to avoid duplicate MSVC C++ runtime symbols when the tokenizer and ONNX Runtime are linked into the same Rust regression-test binary.
- Kept the tokenizer's Oniguruma regex backend enabled, so CLIP tokenization functionality remains available without the conflicting C++ helper path.

## 0.25.2 — 2026-10-26

### Fixed
- Rounded OpenCLIP normalization constants to valid f32 precision so Rust 1.99 Clippy does not reject the semantic preprocessing module.

### Documentation
- Replaced the original project-foundation README with a current end-user and developer guide.
- Documented working features, limitations, privacy behavior, local data locations, iPhone support, duplicate tools, XMP sidecars, AI setup, supported semantic model files, and unfinished work.
- Corrected the in-app File Safety note so it accurately states that original media is read-only while explicit create-only XMP sidecar writes are supported.
- Added this changelog.

## 0.25.0–0.25.1 — Natural-language semantic search

### Added
- Added the text side of the OpenCLIP semantic-search pipeline.
- Added a dedicated semantic text-model slot and tokenizer slot.
- Added local tokenizer support through the Rust tokenizers crate.
- Added 77-token CLIP text input handling.
- Added 512-dimensional normalized text embeddings.
- Added natural-language semantic search over locally stored image embeddings.
- Added Library UI for queries such as “dog at the beach.”
- Added exact compatibility checks for one supported OpenCLIP ViT-B/32 LAION-2B split ONNX bundle.
- Added strict SHA-256 validation for:
  - vision_model.onnx
  - text_model.onnx
  - tokenizer.json
- Added tokenizer JSON import support.

### Changed
- Replaced the earlier generic 224×224 image preprocessing with model-correct OpenCLIP preprocessing:
  - resize/center crop to 224×224;
  - RGB channel-first tensor;
  - OpenCLIP mean/std normalization.
- Normalized semantic image embeddings before storage/search.
- Versioned semantic embeddings by the supported model/preprocessing pipeline so older incompatible embeddings can be marked stale.
- Semantic slots now reject unrelated ONNX/tokenizer assets rather than accepting arbitrary files.

### Notes
- Natural-language semantic search remains experimental until the exact bundle has been exercised end-to-end against representative real photo libraries.
- GPU acceleration is not part of this milestone.

## 0.24.0–0.24.2 — Semantic photo similarity

### Added
- Added local cosine-similarity search over stored semantic image embeddings.
- Added photo-to-photo semantic similarity lookup.
- Added ranked similarity scores in the photo details UI.
- Added embedding decoding and cosine-similarity regression tests.

### Fixed
- Updated embedding decoding for current Rust/Clippy requirements.
- Applied formatting and lint repairs required by the Windows CI gate.

## 0.23.0–0.23.1 — Batch AI indexing

### Added
- Added multi-photo semantic indexing.
- Added a bounded batch size of 500 files.
- Added selected-photo batch indexing controls in the Library.
- Added indexed/skipped/error result reporting.

### Notes
- Batch indexing is local and does not modify original photos.
- AI indexing remains root-scoped and needs more multi-folder end-to-end testing than the normal library scanner.

## 0.22.0–0.22.2 — Persistent semantic embeddings

### Added
- Added ai-embeddings.sqlite under local application data.
- Added persistent semantic image embeddings.
- Added model fingerprint tracking for embedding invalidation.
- Added per-photo “Index this photo” UI.
- Added embedding serialization regression coverage.

### Changed
- Refactored semantic inference so the embedding indexer can reuse the same local model execution path.

### Fixed
- Corrected syntax and formatting issues discovered by Windows CI.

## 0.21.0–0.21.2 — Local ONNX inference

### Added
- Added ONNX Runtime through the Rust ort crate.
- Added CPU-first local semantic image inference.
- Added model loading from Open-Photo’s confined local AI model directory.
- Added input/output validation and embedding-size safety checks.

### Fixed
- Updated the implementation to the current ort environment/session API.
- Fixed formatting issues before the milestone reached a green Windows build.

## 0.20.0–0.20.2 — AI image preprocessing

### Added
- Added deterministic local image preprocessing for AI inference.
- Added a diagnostic Tauri command for prepared image tensors.
- Added regression tests for tensor shape/layout.

### Changed
- Refactored preprocessing so inference can operate on an internal Rust tensor instead of requiring a round trip through the frontend.

### Fixed
- Rust formatting and unused-import lint repairs.

## 0.19.0 — Local AI model removal

### Added
- Added explicit removal of installed local AI model slots.
- Added confirmation before deleting a stored model asset.
- Kept model removal confined to Open-Photo’s private application-data model directory.

### Safety
- Removing a model never deletes original photos.

## 0.18.0–0.18.1 — Local AI model inventory/import

### Added
- Added offline AI model inventory.
- Added semantic, face, and OCR model slots.
- Added manual local ONNX import.
- Added mandatory SHA-256 verification before installation.
- Added a 2 GB per-model safety limit.
- Added create-new behavior so existing model slots are not silently overwritten.
- Added Settings UI for local AI model inspection/import.

### Notes
- This milestone created the model-management foundation only; it did not make face recognition or OCR functional.

## 0.17.0 — Custom smart collections

### Added
- Added workspace-scoped custom smart rules.
- Rules can filter by:
  - minimum star rating;
  - media type;
  - minimum file size.
- Smart rules are stored locally and recompute dynamically.

## 0.16.0 — Persistent duplicate review preference

### Added
- Preferred-copy choices from duplicate review now persist locally between sessions.

### Safety
- Preferred-copy state is advisory only; no file is moved or deleted.

## 0.15.0 — Guided duplicate review

### Added
- Added a non-destructive duplicate-review workflow.
- Added Preview for each candidate.
- Added “Mark to keep” preferred-copy selection.
- Combined exact duplicate and visually similar review results.

## 0.14.0 — Expanded smart memories

### Added
- Added Unrated media smart filter.
- Added This Month in Past Years memory filter.

## 0.13.0 — Dynamic smart collections / memories

### Added
- Added dynamic read-only smart collections.
- Added Last 30 Days.
- Added On This Day.
- Added Large Media.
- Added reusable organization filtering without moving media.

### Notes
- Date-based filters currently use filesystem modification timestamps rather than a complete EXIF/QuickTime capture-date index.

## 0.12.0 — Full-screen viewer and CI improvements

### Added
- Added a full-screen photo details viewer.
- Added keyboard left/right navigation.
- Added Escape-to-close behavior.
- Expanded preview sizing in full-screen mode.

### CI
- Added version-aware workflow run names.
- Added cancellation of superseded workflow runs.
- Added npm cache use to reduce repeated dependency work.

## 0.11.0 — Privacy-first local AI opt-in

### Added
- Added Local AI preference controls.
- AI is off by default.
- Added Automatic/future-GPU and CPU-only preference choices.
- Stored AI preferences locally.

### Notes
- This release added user consent/settings only; inference was not active yet.

## 0.10.0 — Ratings and favorites

### Added
- Added local 1–5 star ratings.
- Added Favorites filter using 5-star ratings.
- Added Rated filter.

### Safety
- Ratings are Open-Photo local application state and do not alter original media.

## 0.9.0–0.9.1 — Library health check

### Added
- Added a read-only indexed-library health report.
- Added counts for:
  - indexed;
  - available;
  - missing;
  - inaccessible;
  - modified.
- Added Settings UI for the health check.

### Safety
- Health checks never delete source files or index rows.
- An unavailable root is treated conservatively rather than as a mass-delete signal.

## 0.8.0–0.8.1 — iPhone media compatibility baseline

### Added
- Added bounded local reading for HEIC/HEIF and common iPhone motion formats.
- Added local HEIC decoding.
- Added inline motion-video preview when supported.
- Added external Windows media-player fallback.
- Added filename-based Live Photo pairing.
- Added Live Photo labeling and motion preview in details.

### Limitations
- Pairing is based on same folder + filename stem, not Apple asset identifiers.
- HEVC playback depends on Windows/WebView2 codecs.
- Large motion clips may require the external player.
- HDR/color/orientation and RAW combinations are not fully validated.

## 0.7.0–0.7.1 — Incremental-index safety regression coverage

### Added
- Added regression tests for missing folders and directory events in incremental indexing.

### Safety
- Strengthened protection against treating unavailable folder events as valid file deletions.

## 0.6.0 — Inaccessible-storage protection

### Fixed
- Prevented inaccessible/stat-failed storage from causing indexed media to be purged.

## 0.5.0 — Watcher event serialization

### Fixed
- Serialized incremental event batches.
- Preserved filesystem events that arrive while an update is already in progress.

## 0.4.0–0.4.1 — Incremental file indexing

### Added
- Added per-file SQLite index updates for filesystem events.
- Added bounded incremental library updates with full-scan fallback.
- Added changed-path event handling.

### Fixed
- Applied Windows rustfmt repairs required for the green build.

## 0.3.0 — Sync observability

### Added
- Added unchanged-media counts to indexed sync summaries.
- Expanded sync reporting for added, updated, unchanged and removed entries.

## 0.2.0 — Indexed-scan performance

### Changed
- Reworked indexed snapshot comparison to compare the complete snapshot in memory instead of querying SQLite once per file.

## 0.1.0–0.1.1 — Desktop foundation and first functional library

This early development line grew well beyond the original scaffold before the project adopted the current green-minor/red-patch versioning policy.

### Desktop foundation
- Created the Tauri 2 + React + TypeScript Windows application.
- Added reproducible app-icon generation.
- Added Windows GitHub Actions installer builds.
- Added enforced Rust formatting, Clippy and regression tests.

### Local browsing
- Added safe recursive local folder scanning.
- Added local thumbnail and detail previews.
- Added grid/list views, sorting and thumbnail zoom.
- Added preview path-boundary regression tests.
- Added virtualized photo tiles and multi-select.

### XMP
- Added explicit create-only XMP keyword sidecars.
- Added bounded tag validation.
- Added protection against overwriting existing sidecars or modifying originals.

### Workspaces and organization
- Added separate workspaces.
- Added multiple folder roots per workspace.
- Added folder/media filters.
- Added workspace-scoped virtual collections.
- Added timeline navigation.
- Added workspace metadata JSON export.

### Map and video
- Added EXIF GPS extraction.
- Added Leaflet/OpenStreetMap photo map.
- Added common desktop/iPhone video indexing.
- Added safe “Open in Windows player” action.

### Durable indexing
- Added SQLite media index.
- Added safe indexed rescans.
- Added exact duplicate detection across workspace roots.
- Added SHA-256 dependency for exact duplicate detection.
- Added persistent local PNG thumbnail cache.

### Sync and watchers
- Added per-sync added/updated/removed summaries.
- Added native recursive filesystem watchers.
- Added debounced indexed rescans with polling fallback.
- Added overlap/stale-scan protection.
- Added optional periodic auto-sync.

### Similar-photo and Live Photo foundations
- Added CPU dHash visual near-duplicate grouping.
- Added exact/visual duplicate review UI.
- Added filename-based likely iPhone Live Photo companion matching.
- Added paired-rename handling to retain virtual collection membership.

### Versioning policy introduced
- Added the current development rule:
  - green build → next minor version for feature work;
  - red build → patch repair;
  - major version reserved for approved main release.
- Added CI validation that package.json, Cargo.toml and tauri.conf.json use the same version.

## Planned / not yet completed

The following remain active roadmap items and should not be represented as finished:

- local face detection and recognition;
- People review, merge and naming;
- XMP face-region export;
- OCR;
- GPU acceleration;
- background AI queue/progress/pause/resume;
- full EXIF/QuickTime capture-date timeline;
- complete RAW preview pipeline;
- hierarchical XMP editing/merging and broader metadata portability;
- non-destructive photo editing/export presets;
- safe delete/trash workflows;
- offline map tiles;
- metadata-based Apple Live Photo pairing;
- Immich API synchronization and person mapping;
- formal public licensing/release.
