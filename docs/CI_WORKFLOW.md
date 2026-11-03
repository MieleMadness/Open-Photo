# Windows GitHub Actions: fast PR checks and full installer validation

The Windows workflow lives in `.github/workflows/windows.yml`.

## Every pull request: fast validation

The `Frontend checks` and `Rust checks` jobs execute on every PR update. They validate:
- Version alignment, npm tests, and the production frontend bundle
- AI model and FFmpeg/video-engine resources with integrity checks
- Rust formatting, Clippy with warnings treated as errors, and native Rust tests (including video preview/export tests)

A passing PR is **not** proof that its installer or installed-app lifecycle was tested. PR jobs intentionally skip the costly NSIS build and installed-app acceptance by default.

## Full validation: before a release or when needed

Run the full workflow by any of these routes:
1. Push to `main` (automatic full verification).
2. From GitHub **Actions → Open-Photo Windows Desktop Build → Run workflow**, select the desired branch and run manually.
3. Add the `full-windows-ci` label to a PR. This re-triggers validation and enables the Windows installer job for the labeled PR. Removing the label returns later PR updates to fast checks.

Full verification runs frontend and Rust checks, then builds the Windows NSIS installer, runs release video export acceptance, validates installed AI and uninstall behavior, and uploads the installer and JSON reports. **Before merging/releasing**, require a full green workflow on the exact commit to be shipped, not merely a fast PR pass.

## Why this is faster

Previously every PR waited for a **serial** installer job *after* Rust and frontend checks, and the installer job repeated model and video-engine preparation. Most development changes only need the frontend + native correctness gate; keeping installer packaging for deliberate release checks avoids this extra wait. Both Rust and installer jobs now cache the checksum-verified FFmpeg package under `src-tauri/target/ve`; cache keys change when the preparation script changes. AI models and npm/Rust dependencies also use their existing caches.

## Seeing progress

GitHub Actions shows named phases separately: model verification, video-engine preparation, formatting, Clippy, native regression tests, and full installer acceptance. Expand a running step to inspect its live logs. The Rust job also writes a concise explanation of the verification mode to its **Summary** tab, including how to request full CI.

Cache warmup and large native Rust builds can still take time. Exact savings depend on runner scheduling, cache hits, and the size of code changes; no fixed run time is guaranteed. The first run with a new FFmpeg cache key may be slower while populating the cache.

## Debugging red builds

Use the failed step, not the overall run number, to identify the root cause. Do not call canceled runs "red": concurrency cancellation is expected when a new commit is pushed to the same PR. Avoid pushing a new commit while measuring a run, because it invalidates the comparison.
