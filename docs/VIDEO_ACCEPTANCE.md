# Repeatable video acceptance (0.54.6)

Phase 6 has an automated native export baseline. It is a measurement and correctness check, not completion of installed-app acceptance.

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
