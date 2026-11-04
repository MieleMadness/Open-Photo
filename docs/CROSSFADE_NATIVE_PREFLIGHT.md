# Experimental native sequence crossfade preflight

This change is **test-only** and does not activate an editor feature.

`crossfade_preflight::plan_sequence` accepts an existing validated schema-v3 `Sequence` and a separate proposed array of adjacent overlap boundaries. No saved project transitions are mutated, persisted or migrated. It calls the exact existing `video_sequence_render::frame_allocation` function, now accessible within the crate, to ensure native overlap boundaries match the active exporter’s cumulative clip-frame rounding.

## Guards currently enforced

- Only 30 or 60 fps; at least two and no more than 200 clips; at least one genuine overlap; no more than two hours total.
- Existing dip-to-black transitions, titles, independent audio tracks and sources are rejected until a clock migration policy is implemented.
- Photos are rejected until a mixed-media compositor can be verified.
- Each proposed overlap must join adjacent clips and preserve two clean frames per clip, including consecutive transitions.
- The normalizer must provide a stereo 48 kHz audio stream for **every** clip. The preflight marks clips with absent audio, muted video, or zero source gain so future normalization can insert silence.
- Returns expected video frames, expected 48-kHz audio sample count, normalized clip frame counts and an FFmpeg graph plan. It **does not** run FFmpeg, validate actual media paths, create temporary files, or alter the existing renderer.

## Outstanding gates for production use

A real renderer needs to safely normalize every video/audio stream to exact expected counts, synthesize silence where required, enforce cancellation and source path protection, run the bundled FFmpeg graph, verify output frame/sample counts and preview/export parity, then introduce schema migration and editor controls for selectable crossfades. This preflight is a guardrail, not a replacement for those stages.
