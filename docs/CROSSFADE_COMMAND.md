# Experimental crossfade FFmpeg invocation boundary

`src-tauri/src/crossfade_command.rs` is registered under `#[cfg(test)]` only. It builds a bounded argument vector from the previously validated `PreparedCrossfade` graph and an ordered list of normalized Matroska clip part paths.

## Safety contract

- Only 30 or 60 fps, 2–200 inputs, a two-hour output limit and an explicit size cap no larger than the existing export maximum.
- Inputs must be distinct absolute `.mkv` paths; output must be an absolute `.mp4` path. Uses `-n` to refuse overwrites.
- The command uses internal graph labels and frame counts, explicit stream mapping, 48 kHz stereo AAC, disabled metadata/chapter copying, and a `file,pipe` protocol whitelist. It does not invoke a shell.
- The builder **does not** prove that inputs reside within owned temporary storage, exist, have the promised normalized stream properties, or were fingerprinted. The eventual caller must enforce those conditions, cancellation, temporary ownership, output validation, and cleanup before enabling execution.
- This module **does not execute FFmpeg**, route any user export, or change the active editor. A future stage should use the existing cancellable `video_engine::run_until`/progress runner and source revalidation rather than introducing a new process runner.

## Next gates

1. Verify scratch-directory ownership and normalized clip stream/audio sample counts.
2. Add a cancellable test-only invocation with bundled Windows FFmpeg and strict post-encode frame/audio verification.
3. Add fault-injection cases for cancel, missing part, wrong stream shape, output collision, and insufficient space.
4. Only then design native renderer integration, persisted transitions and editor UI, followed by installed-Windows acceptance.

## Bundled Windows engine execution fixture

A Windows-only regression exercises the experimental command through the existing cancellable `video_engine::run_until` process runner. It creates two 60-frame 30fps 96×64 normalized Matroska parts with 48kHz stereo PCM, plans a 15-frame overlap and expects exactly 105 output frames, 3.5 seconds and an AAC audio stream at 48kHz. The fixture reruns the command against an existing output and compares the complete output bytes to verify `-n` prevents modification; FFmpeg may exit successfully even when it skips an existing file. This is synthetic test execution, **not** production source-verified rendering, and still requires cancellation, source fingerprint revalidation, exact decoded sample count and scratch ownership testing.

## Test-only cancellable scratch execution and validation

The experimental `execute_in_scratch` helper now combines the scratch path gate, bounded command generation and existing cancellable bundled FFmpeg process runner. It refuses a pre-cancelled job, checks cancellation after rendering and probing, and removes an incomplete MP4 whenever execution or output verification fails. Successful synthetic runs verify nonempty and bounded output size, exact video frame count, duration and 48kHz audio duration. Windows fixture cases cover a pre-cancelled execution and an intentionally corrupt normalized MKV, asserting that neither leaves an MP4 behind.

This is **not** connected to production export. It now invokes the existing cancellable full-output decode check before retaining an MP4, but does not yet prevent scratch-path replacement races, verify original media fingerprints across the run, or guarantee exact AAC sample count. An installed Windows acceptance and strict publication boundary remain mandatory before enabling user crossfades.
