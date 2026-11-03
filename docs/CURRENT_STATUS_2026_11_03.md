# Current development status and next work — 2026-11-03

This is a **development-branch** snapshot at source version **0.56.63**. It reconciles `docs/DEVELOPMENT_PHASES.md`, `CHANGELOG.md`, `docs/MILESTONES.md`, and `docs/VIDEO_ROADMAP.md`. A source implementation is **not** proof of a passing GitHub Actions build, signed release, or installed-Windows acceptance.

## Branch and release truth

- The active application work lives on `feature/desktop-v0.1.0` (currently the head of PR #2); `main` still contains the original project foundation and is **not** the latest functional application baseline.
- Keep unverified application work on its development branch. Do not merge PR #2, older stacked PRs #3/#5/#6, or this documentation update into `main` without reconciliation and successful validation. Branch and PR state should be rechecked immediately before merge.
- The Windows Desktop Build workflow is the primary installer CI. Confirm the **exact source commit SHA**, run conclusion, completed jobs and artifact before calling an installer green. Historical green workflow IDs do not certify later versions.
- Development version 0.56.63 means *source checkpoint*; do not describe it as a published or installed release.

## Implemented in source (hardware acceptance still separate)

| Capability | Source status | Qualification |
| --- | --- | --- |
| Multiple local folders, rebuildable SQLite library, read-only scanning, manual Sync | Implemented | Offline/index correctness and missing-drive recovery need installed testing |
| Thumbnails, browsing, timeline, memories, virtual/smart collections, exact/near duplicate review | Implemented | 10,000+ real-media Windows acceptance is mandatory before 1.0 |
| Image previews, Live Photos, HEIC/HEIF support, GPS maps and zoom clusters | Implemented/partial by format | Real iPhone HEIC/MOV samples, offline map privacy, polar/dateline cases require acceptance |
| Local semantic AI, incremental jobs, People/OCR, CPU/DirectML paths | Experimental | Model accuracy, hardware and packaged runtime require testing; no cloud recognition required |
| Timeline editing and joined video/photo preview/export, saved projects, recovery | Implemented in source | Real Windows decoder, A/V sync, cancellation and export checks remain |
| Timeline-only video trim, sampled filmstrips, adjustable preview divider | Implemented in source | Keep trim gestures on the timeline, not a competing source-trim UI |
| Remembered global video volume, lazy muted video/Live Photo hover preview, five accent colors | Implemented in source | Validate preferences and synchronized players in installed app |
| Social canvas presets, safe-area framing aid, timed native-rendered titles | Implemented in source | Safe-area guide is preview-only, not a platform guarantee; output font and placement checks remain |
| Standalone MP3/WAV/FLAC/M4A import and native mixed audio tracks | Implemented in source at 0.56.59+ | Fingerprint validation and relink included; installed listening/export tests remain |
| Measured 64-column audio waveform and accessible click/drag/keyboard seeking | Implemented in source at 0.56.62–0.56.63 | Only the **first 120 seconds** are sampled; waveforms are not a complete long-project display |

## Next development order

1. **CI/source integrity first.** Identify PR #2's exact head SHA, its latest Windows Desktop Build run, any failing jobs, and conflict with parallel Codex work. Fix the first reproducible red check before layering on new native rendering changes. Keep build labels aligned with the actual project version.
2. **Phase I audio interaction and correctness.** Add non-destructive audio trim handles anchored to the existing timeline/project clock (not duplicate source trim). Preserve per-track offset, duration, source fingerprint, gain and fades; cancel stale gestures; implement pointer, keyboard, undo/redo and project-reopen coverage. Keep seek/scrub a preview action until the user commits a trim.
3. **Phase I transitions.** Start with one hard-cut-compatible short transition (such as a crossfade) only after defining persistent transition schema, matching preview/export native render, invalidation/relink behavior, undo, and test fixtures. Avoid UI-only transitions that export differently.
4. **Waveform scaling.** Replace the 120-second-only excerpt with bounded windowed sampling/cache keyed by source identity plus excerpt/time resolution; never decode the entire multi-hour audio source into the UI. Preserve existing 120-second fallback until validated.
5. **Polish and media acceptance.** Test captions/titles, standalone audio, project/single source preview, original read-only preservation and short installed Windows A/V output. Then complete Phase H real-iPhone GPS cases and deferred Phase G 10,000+ library gate before 1.0.
6. **Separate later milestones.** Phase J soundtrack browsing/licensing/user experience follows stable Phase I audio foundation; Phase K encrypted vault requires threat model and key/recovery design before implementation. Immich connector is not implemented.

## Nonnegotiable product decisions

- Original photo/video/audio files are read-only. Project edits create new outputs and must not overwrite source content or unrelated XMP.
- All native decoding, inference, model/data access and file bindings are local and scoped to registered workspace roots; verify fingerprints before rendering.
- Video trim is controlled from the shared timeline; a seek action changes **playhead**, not durable trims.
- The preview is driven by the assembled project playhead. Source-inspection mode must be clearly distinguished from rendered-project playback.
- Respect Windows reduced motion and keyboard accessibility; synchronize remembered volume across viewers.
- Real 10,000+ Windows media performance, physical drive/disk failure recovery, signed two-version updater acceptance and installed GPU/AI playback tests remain required **before 1.0**, even when intentionally deferred during feature development.

## Definition of green for the next change

- Formatting, TypeScript build, unit/component tests, native tests and strict lint pass for the **same commit**.
- Windows Desktop Build installer job succeeds and artifact corresponds to that SHA.
- New waveform/transition behavior has negative/edge tests for unavailable or modified sources, zero/over-limit duration, and project state reopening.
- Document installed-manual-test results separately; do not infer installed-Windows passing from CI.
