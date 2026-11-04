# Native crossfade normalized part contract

The experimental crossfade pipeline must verify each scratch video part before FFmpeg's multi-input graph is allowed to use it. This test-only helper validates **parsed ffprobe metadata**, not original media hashes or filesystem ownership.

- Every part reports the exact allocated project frame count and the same nonzero canvas.
- Frame rate is exactly the project rate (30 or 60 fps), without VFR warnings; the format is H.264/yuv420p with no rotation or HDR.
- Each part reports **one 48 kHz stereo PCM float audio stream**, including silence generated from a source with no audio. Reported audio duration, when available, must match the allocated frames.
- Missing tracks, mismatched frames, canvas, audio rate, HDR and VFR metadata fail closed.

A future render integration must call the real `video_probe::probe` on every existing normalized MKV, enforce scratch ownership, verify source fingerprints before/after encode, decode the final MP4, and prevent path replacement races. Metadata validation is not exact decoded audio sample counting and does **not** make crossfades available in the UI.

The test fixtures use synthetic ffprobe JSON to test stream and frame checks without running a costly full Windows media encode for every invalid case.
