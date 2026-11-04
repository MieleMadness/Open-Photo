# Phase I — Native FFmpeg crossfade filter-graph planning

This is a **test-only** follow-up to the merged frame-clock foundation ([Issue #20](https://github.com/MieleMadness/Open-Photo/issues/20)). The active exporter and preview keep using the validated hard-cut/dip-to-black path.

## Current increment

`src-tauri/src/crossfade_graph.rs` consumes a validated `crossfade_clock::Clock` and deterministically builds a candidate FFmpeg `-filter_complex` for **already normalized** video/audio parts.

- Every input must be a zero-timestamped, equal-sized, equal-frame-rate YUV420p video with a zero-timestamped 48 kHz stereo audio track; silent clips need synthesized silent audio before joining.
- A hard cut is a video and audio `concat` filter; a positive overlap is a video `xfade` plus audio `acrossfade` filter.
- The `xfade` offset uses the accumulated project length **minus** overlap duration. This prevents successive transitions from drifting away from the integer project frame clock.
- Filter labels and values derive from internal numeric indices and validated clock data. The planner accepts no user-supplied paths or filter strings.
- Initial audio envelope is `acrossfade` with `tri` curves (linear amplitudes). This **intentionally differs** from the equal-power gain *proposal* in the earlier TypeScript sampling prototype. The actual production policy must be settled with native listening fixtures before the renderer is enabled.

## Not implemented by this patch

No production code calls this graph planner (`#[cfg(test)]` only). There is no persisted crossfade selection, source handle policy, FFmpeg command invocation, source protection acceptance, audio sample-count verification, UI playhead retiming, installer GUI review or real preview/export parity yet.

## Required next integration

1. Create same-size, exact-frame-count and exact-sample-count normalized video/audio clip parts (include silence where source audio is absent); reject unsupported HDR and VFR corner cases until tested.
2. Wire the graph to a separate native crossfade compile/encode path with explicit bounded limits, cancellation and safe temporary files; do not replace the known-good hard-cut exporter in place.
3. Verify FFmpeg graph support using bundled engine; decode and measure each frame of normalized fixtures and audio continuity.
4. Test multiple consecutive transitions, mixed hard cuts/crossfades, 30/60 fps, muted audio and mixed still/video sources before enabling UI.
5. Introduce a versioned project transition format, TS/Rust validators, undo/redo and consistent titles/music timing; perform installed-Windows preview/export acceptance.

This planner is groundwork and is **not a working end-user crossfade feature**.

## First real FFmpeg fixture (development-machine smoke test)

The planner's initial `settb=AVTB,setpts=PTS-STARTPTS,format=yuv420p` chain lost constant-frame-rate metadata when passed into FFmpeg 7.1's `xfade` filter, which rejected the stream as rate `1/0`. The corrected chain is `settb=AVTB,setpts=PTS-STARTPTS,fps=30,format=yuv420p` for a 30 fps project (or 60 for 60 fps): applying `fps` **after** resetting timestamps restores explicit CFR at the transition input.

A **local FFmpeg 7.1.5 fixture**, using two synthetic, normalized 64×64 video clips (90 red frames/3 seconds and 120 blue frames/4 seconds), with 48 kHz stereo silent audio and 15 overlapping frames, produced **195 output video frames at 30 fps, 6.5 seconds of video, and 6.5 seconds of audio** using the corrected graph. This verifies that sample graph syntax works for one synthetic video/audio overlap. It does **not** yet validate Open-Photo's bundled Windows engine, 60 fps, actual user media, chained transitions, cancellation, source protection, or installed-app preview/export behavior.
