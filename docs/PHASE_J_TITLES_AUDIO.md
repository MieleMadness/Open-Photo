# Phase J phase 2 — gated titles and PCM composition

`PreviewRequests::av_next` now owns one request from verified source decoding through visual composition, global title rendering and an exact audio window. `av_current` revalidates the typed result at final consumer handoff. This remains Windows/test-only; no production IPC, editor activation, migration or export routing was added.

## Composition policy

- Titles are rendered once **after** dissolves/dips, in saved project order. Their quantized start is inclusive and end exclusive. The existing Windows native style is retained: Arial, white text, black border, canvas-relative font size and percentage positioning. Text goes through private UTF-8 files with expansion disabled, never through interpolated filter syntax.
- Audio is **48 kHz stereo float PCM**, exactly 1,600 samples per channel at 30 fps or 800 at 60 fps. Added tracks retain absolute project positions, including subframe starts. Linked clips use their source trim plus normalized clip-local offset. Source audio timestamps are normalized/resampled before exact sample slicing; short windows are explicitly padded with silence.
- Crossfades use the native planner's **sample-centered linear envelopes**, not the older TypeScript equal-power proposal. Each source's saved gain and independent fades apply before summing. This keeps identical correlated sources at constant amplitude across a linked crossfade; it deliberately does not promise constant power for unrelated material.
- Muted/silent intervals do not start audio decoders. Added tracks are mixed once and do not inherit clip overlap envelopes. All layers sum in double precision, then the final float output is clamped to [-1, 1]. `clipped_samples` records affected interleaved channel samples; there is no hidden normalization or per-source clipping.

## Safety and bounds

Audio sources are resolved from the verified project, checked against stored byte/hash bindings and held with Windows read pins through result ownership. Repeated sources pin once; conflicting bindings fail. Independent audio gets its own pins in addition to the visual source pins. Silent-only results require no new audio pin.

The gated implementation allows at most 32 active audio/title layers, 128 MiB per selected audio source and source sample positions within the first ten seconds. These are prototype limits, not changes to production rendering. Video/photo geometry retains phase 1 bounds. Source decoders run sequentially, use restricted demuxers/protocols, bounded output/allocation settings and 30-second child-process deadlines. This is not a whole-job deadline or whole-app memory claim.

Temporary title inputs/text are in an owned scratch directory, fingerprinted and pinned during FFmpeg execution; pins release before guarded cleanup. Cancellation is checked throughout source/part processing and before completion. A superseding request cannot reuse a worker slot until the old operation acknowledges completion. Stale native project/session data prevents handoff. Original media is never opened for writing.

## Verification and remaining integration

Native regressions use actual bundled FFmpeg, AAC-linked sources, uncompressed stereo WAV and video/photo fixtures. They cover title placement and exclusive end, silent windows, sample-centered gains, clipping counters, linked overlap continuity, independent fade-out, muted layers, duplicate-source mixing/pinning, source corruption, cancellation, 60-fps windows and a five-sample subframe track start. PCM amplitude assertions on the uncompressed fixture are tighter than a single envelope-sample increment.

Reference parity here is the explicit gated title/PCM recipe and digital sample/pixel checks. The legacy production exporter still uses its older YUV transition/title order and audio envelope implementation. Phase 3 must connect/reconcile that route with these policies and verify end-to-end encoded preview/export parity. Hardware listening, installed playback, screen-reader and release acceptance were not performed here; they are not implied by digital tests. No human listening claim is made.

Evidence is under `../../audit-output/phase-j-av-*`; final native verification is `phase-j-av-verified.log`, denied-warning Clippy is `phase-j-av-clippy-verified.log`, and the TypeScript/Vite build is `phase-j-av-build.log`. Earlier failed runs are retained as development evidence. The 0.56.86 installer predates this test-only engineering increment and has not been rebuilt, installed or published for it.

Final local verification on November 7, 2026: **439 native tests passed**, eight opt-in acceptance modes skipped; denied-warning Clippy, Rust formatting, TypeScript, Vite production build and diff whitespace checks passed. Ten phase 2 regressions were added. No production release, installed acceptance or human listening result is implied.
