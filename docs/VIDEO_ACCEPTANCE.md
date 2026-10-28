# Repeatable video acceptance (0.54.10)

Phase 6 has an automated native export baseline. It is a measurement and correctness check, not completion of installed-app acceptance.

Source-window reference inspection decodes with strict FFmpeg error handling and streams frame CRC records, retaining the original presentation timestamps and demux time base. Preroll begins before the selected range; inspection stops after the next decoded frame beyond its end, so reordered frames are drained. The reader retains only selected timestamps, caps each line and the frame count, and rejects malformed or incomplete records. This replaces FFprobe interval seeking, which intermittently returned no frame records on the Windows CI runner. Boundary fixtures compare counts and timestamps with an independently probed complete short-source decode. This changes acceptance inspection, not the editor's rendering pipeline.

## Run the baseline

On Windows, prepare the pinned engine (`npm run prepare:video`) and use a Rust/MSVC development environment with the same toolchain as the repository. Run:

```powershell
./scripts/accept-video.ps1 -MachineLabel 'Reference desktop'
```

The script creates a fresh directory under `src-tauri/target/` and prints its location. Use `-OutputDirectory 'C:/reports/new-run'` to choose a new directory. Existing directories are rejected, so a failed run cannot accidentally present an earlier successful report. The test must run alone in release mode; the script selects the exact ignored test with one test thread. Normal debug tests skip this measurement.

The fixture is generated locally: eight seconds of 1920×1080, 30 fps test video and 48 kHz stereo audio. The saved project keeps 0.37–7.61 seconds, crops its width, rotates 180 degrees, adjusts gain, and exports through the same native pipeline as the app. Fixture creation is excluded from export timing. The real destination-space query, source fingerprint verification, stream/timing validation, full output decode, staging, and publication all run. Assertions check output dimensions/duration/size, source preservation, required stages, temporary cleanup, and an available memory measurement.

## Reports and interpretation

- `video-export.json`: app/engine versions, fixture parameters, total export wall time, media seconds per wall second, stage start times, output properties, checks, and the maximum individual native child's peak working set.
- `environment.json`: a supplied machine label, CPU description, logical processor count, OS/architecture, and UTC recording time. It does not collect a hostname, user name, source path, or personal media.

Memory is the maximum Windows-reported `PeakWorkingSetSize` for an individual owned FFmpeg or ffprobe process. The test samples process counters in the existing 20 ms wait loop. It is not a sum of simultaneous processes or a measurement of Rust, WebView2, GPU allocations, or whole-app memory. Instrumentation is compiled only for Windows test builds. The export wall time includes source verification, encoding, output validation/decoding, and publication, with stage offsets for context.

These results are one short synthetic baseline, affected by machine load, filesystem cache, and hardware. No numeric speed or memory budget is enforced yet; correctness failures fail the test. Establish a named reference-machine baseline before choosing budgets. Comparing two different runner machines does not establish a regression by itself.

The Windows installer workflow runs this test after building the installer. A failure blocks successful completion of that job. Successful runs upload `open-photo-video-validation-<run number>` containing both JSON reports, separately from `open-photo-windows-<run number>`.

## Remaining Phase 6 gates

Use the packaged app on a named Windows reference PC and record results for:

- Real phone/camera SDR clips: audio/silent, portrait rotation, variable frame rates, and supported codec fallback. Verify the chosen trim, transforms, and audio by playing the exported file. Confirm playback/export with no system FFmpeg installation.
- Hour-scale recordings: check audio/video alignment near the beginning, middle, and end. The short synthetic baseline does not establish long-recording synchronization.
- Save/reopen, relink, undo/redo, cancellation, editor/app closure, and recovery after an interrupted export. Browser marker placement remains approximate.
- Long paths, removed/unplugged drives, actual disk-full/quota/write failures, and a destination created while export runs. Existing automated failure injection does not replace these hardware checks.
- App startup and seek latency, export speed, and whole-app/GPU memory under realistic library and media sizes. Record machine specifications, workload, measurements, and then choose explicit performance budgets.

HDR tone mapping and waveform UI remain deferred. Phase 7's basic sequential multi-video editor remains planned in [the roadmap](VIDEO_ROADMAP.md).

## Opt-in real footage

Use a source file explicitly chosen for testing. Outputs go into a fresh separate directory; the source is fingerprinted and left unchanged:

```powershell
./scripts/accept-video.ps1 -Source 'C:/test-media/gameplay.mp4' -OutputDirectory 'C:/reports/new-real-run' -MachineLabel 'Reference desktop'
```

This runs the exact ignored optimized real-media test instead of the synthetic baseline. It requires an SDR clip at least 4.61 seconds long with a supported source canvas. It exports 0.37–4.61 seconds at 75% audio gain to separate source-resolution and 3840×2160 fit canvases. Each export uses the real validation, full-decode, source-reverification, and publication pipeline. Project JSON copies reference the selected source relative to its containing library folder; open them with that folder selected in the app. This does not test GUI interaction or establish perceptual quality.

The real report includes the chosen filename, source properties/hash, and per-export results. Keep it and its media local unless you intentionally choose to share them. CI runs only synthetic media and does not upload this local footage. A 3840×1080 recording can retain its ultra-wide source resolution or be fitted into UHD with borders; it is not native 3840×2160 footage.

## Hour-scale source windows

```powershell
./scripts/accept-video.ps1 -Source 'C:/test-media/hour-recording.mp4' -LongRecording -OutputDirectory 'C:/reports/new-long-run'
```

This optimized, opt-in gate requires an SDR recording at least one hour long, square pixels, and a supported source canvas. It exports 4.24-second windows near the beginning, middle, and end to separate files at source resolution. Each window compares decoded video frame counts/timestamps with a bounded source inspection, checks independent stream timing through the production validator, and fully decodes the output. The original source hash must remain unchanged. Reports and portable project copies stay in the chosen fresh output folder.

The ordinary native suite includes the same three-window check using generated hour-scale media. Source inspection uses a bounded-memory packet scan to select a seek point between earlier random-access packets, avoiding exact-boundary forward seeks. With fewer than two earlier points it starts from the beginning without an initial seek. It then streams decoded presentation timestamps until the next frame is beyond the selected range, then stops the owned probe process. Completed windows ignore all later probe output, including a final partial line during termination. Packet-boundary stopping can miss buffered B-frames; a reordered-frame regression compares the bounded scanner with a complete short-source decode. Reads are limited to 4096 bytes per line, 50,000 selected frames, and 180 seconds. Fixture generation has a separate 120-second limit.

These checks establish short-window seeking and timing at distant source positions. They do not establish complete hour-long encoding, perceptual A/V synchronization, GUI seeking latency, or installed-app playback.

The player now supports Go to time using seconds (3868.125), minutes:seconds (64:28.125), or hours:minutes:seconds (01:04:28.125). Invalid or out-of-range entries show an error and preserve the current position. Browser seeking remains approximate; saved trim positions continue to use rational source ticks.
