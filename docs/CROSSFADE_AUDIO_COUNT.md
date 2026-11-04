# Experimental AAC audio sample-clock verification

A native crossfade project needs exactly `expected_audio_samples = expected_video_frames × 48000 / fps` **presentation samples**.

AAC decoders may expose padded decoded samples even when the MP4's presentation duration is correct. For example, a two-second 48kHz AAC fixture locally yielded 96,256 decoded frame samples, while packet durations minus 1,024 priming samples yielded the correct 96,000 presented samples. **Decoded frame counts alone must not be used as an exact export acceptance gate.**

The test-only module provides two deliberately separate checks:

- `sum_decoded_samples` measures raw decoded frames and rejects malformed/overflowing data, but does not make a valid presentation-timing decision by itself.
- `presentation_samples_from_probe` and `verify_presentation_samples` read bundled ffprobe audio packet durations plus skip/discard padding, verify a 1/48000 time base, and compare the resulting presentation sample total to the expected project clock. A two-second bundled-Windows AAC fixture checks the actual behavior.

The current runner caps ffprobe output at 4 MiB and will fail closed on long projects until a streaming, bounded-memory packet inspector is implemented. A full video/audio decode and source fingerprint revalidation remain separate requirements. This module is still `#[cfg(test)]` and does not change production exports.
