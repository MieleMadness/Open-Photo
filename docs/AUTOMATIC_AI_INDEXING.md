# Automatic local AI indexing — prepared 0.56.23

Local code only; no release or deployment has been performed.

Turn Local AI on in Settings to process media already present in the active workspace's saved Library snapshot. Startup resumes known work. **Sync remains manual**: adding a folder or clicking Sync makes new/changed media eligible. Indexing never watches folders or discovers unrelated files in the background.

## Dashboard

Settings → AI indexing shows indexed, pending, failed, excluded and unsupported **source-file** counts. A source is indexed when all its applicable services complete: image meaning/OCR/People for supported stills; representative-frame meaning for videos. Partial completion is included in the pending/failed totals, not an extra file category. A Live image's still and motion are separate source files in these counts, while the Library keeps its paired presentation.

Each registered folder has **Don't index / Index folder**. An excluded ancestor also excludes its descendants; enable that ancestor before enabling a nested folder. Exclusion stops queued work and hides existing AI evidence in this workspace; it does not remove the folder, affect ordinary metadata search, delete media or erase another workspace's indexes. Existing shared cached indexes remain available if the folder is re-enabled. Offline folders are held pending with an explanation rather than generating a failure for every file.

**View failures** shows file, service, error and attempt time. Search and page through the full failure list, retry one file, or use **Retry failed / resume**. Corrupt files are not retried endlessly. Missing/incompatible models pause the service with one actionable error. Reconnect an unavailable folder before resuming. Switching AI off pauses scheduling immediately, even if saving preferences fails; the current photo/frame may finish. Completed data and person names are kept.

## Architecture and safety

`ai_index.rs` owns one native worker, a durable SQLite queue (`ai-work.sqlite`) and per-workspace folder policy. React configures/observes it; React unmounts do not own inference lifetime. Jobs record source root/path, modified time, size, model/pipeline key, service, status and bounded errors. Successful/failed state survives restart; interrupted units resume. Changed inputs/pipelines invalidate relevant jobs. Generation and source-version fences reject obsolete completion after pause, workspace changes, exclusion or refresh.

Existing image/OCR/People decoders, models and stores are reused. A service pass retains its model session across files. A shared inference lane gives foreground search the next slot before another background unit. Source metadata is checked before and after processing, and SQLite results publish transactionally. Very extreme image aspect ratios are rejected before an oversized AI resize allocation. All source files stay read-only.

Queue and model stores are local application data. Include `ai-work.sqlite` when backing up application data if you need its exclusion/retry history; the existing metadata-JSON export is not a complete AI-database backup. Normal uninstall keeps app data unless the explicit removal option is chosen.

## Video content

`video_semantic.rs` samples 3–48 positions spread across a video's duration (approximately one per second for short clips), removes near-identical representative frames, and embeds them with the existing pinned vision model. Only one bounded frame is decoded at a time using the bundled FFmpeg, with process timeout/cancellation, protocol restrictions and a 640-pixel edge limit. HDR conversion is not invented; unsupported HDR is reported. The current duration bound is two hours; long-form optimization remains deferred.

Each frame is a scheduling unit, allowing foreground search between frames. A private staging table persists completed frame work; it is never queried as a finished video. The final transaction replaces searchable frames and marks the video current. Cancellation/restart can resume the matching source/pipeline, and changed inputs discard incompatible staging. The sampler stores positions **near** decoded frames; sparse tails that need a first-frame fallback record that frame's own position rather than an invented late timestamp.

Unified search creates one text vector and ranks image and video evidence together, retaining at most five matching times per video and a global bounded media candidate set. Workspace/root/freshness/exclusion filters apply before provider caps. Windows/Unicode exclusion policy is resolved in Rust, not SQLite's ASCII-only case conversion. Verified Live motion evidence is projected to its paired still, preserving timestamps. **Play near…** opens/seeks the shared player near a match.

Sampling can miss brief appearances, and model similarity is not a confidence percentage. Native content scans are bounded to 50,000 compatible rows per provider, with truncation disclosed; large-library acceleration remains Phase G. HEIC AI decoding, video OCR/faces and model accuracy/GPU acceptance remain outside this patch.

## Local verification

Queue tests cover restart, exclusions, cross-workspace ownership, changed inputs/models, counts and obsolete completions. Search tests cover Unicode exclusions, committed/current video evidence, timestamps and Live projection. A sub-second three-scene fixture verifies extraction, cancellation and source preservation. The explicitly run CPU-model acceptance verifies frame staging/resume/atomic publication with isolated SQLite data. These complement, rather than replace, an installed-app background-indexing/search smoke test.
