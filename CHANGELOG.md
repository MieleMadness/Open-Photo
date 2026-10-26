# Changelog

This file records the major Open-Photo feature milestones, important add-ons, safety changes, and notable build repairs.

Open-Photo is still in **0.x active development**. During this phase:

- a green predecessor advances to the next **minor** version for feature work;
- a red build is repaired with a **patch** version;
- patch releases often contain formatting, lint, regression, or compatibility fixes rather than new user-facing features.

The detailed Git commit history remains the source of truth for every individual change.

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
