# Single-video MP4 export (0.54.4)

Create or open a video project, choose **Preview edits** if desired, then **Export video MP4** and a new output filename. The app waits for autosave and submits a separate snapshot of the saved trim/transform/audio/canvas settings. Subsequent edits do not alter a running export. Export continues if the editor closes; the app-wide panel shows the destination, phase, progress, elapsed time, cancellation, and completed-file opening. Only one export is accepted at a time, and native processing is serialized with preview jobs. Normal app closure stays open while an export is active; finish or cancel it first.

## Supported preset and policy

- SDR H.264, 8-bit YUV420P, target video bitrate 12 Mbps, with optional AAC stereo at 48 kHz / 192 kbps. Variable presentation timing is preserved through source encoder time bases rather than forced constant frame rate.
- Use the saved project canvas, or limit it to 1920 by 1080 (landscape) / 1080 by 1920 (portrait), preserving a smaller canvas. Source rotation is applied before project crop/rotate/flip. Fit/fill and audio gain use the same graph as edited preview; preview only reduces the canvas and video bitrate.
- Audio is removed when muted or absent. Otherwise use the first source audio track as stereo; multiple source tracks require explicit confirmation. Zero gain retains a silent AAC track. Subtitles, extra audio, chapters, and source metadata are intentionally omitted. The export summary discloses this before choosing a destination.
- Detected HDR is rejected. This release does not promise HDR tone mapping, source metadata preservation, or color-managed mastering.

## Validation and publication

Source identity is checked against the project's full SHA-256/length before encoding and again after processing. A conservative space estimate checks the destination's available quota on Windows. An existing destination is never replaced. The encoder writes only a uniquely named, owned staging file in the chosen destination folder, using explicit MP4 formatting.

Before publication, ffprobe checks duration, dimensions, rotation, H.264/YUV420P format, size, and the audio policy. Duration tolerance is two average-frame durations with a 100 ms floor for VFR/container padding. Audio start is checked within 50 ms, and end within 100 ms when source timing metadata is available. A second FFmpeg pass decodes all output video/audio with errors treated as fatal. The source is reverified, the staged file is synced, and the final path is published without overwrite. A file created by someone else while export runs wins; it is left untouched and export fails cleanly.

Encoding progress occupies 0–90%, output decoding advances toward 99%, and only successful publication reaches 100%. Source checking and queue waits can be indeterminate. Once publication starts, cancellation is disabled so the result has one clear commit boundary. Successful output can be opened in the Windows player. Dismissing a result removes its status, not the exported file.

## Cancellation and limits

Cancel during queueing, source checking, encoding, or validation. Native work is terminated/reaped and only the owned staging file is removed. Cleanup failures report the temporary path; they never delete an existing destination or source. Windows process jobs also contain native children. Abrupt termination/power loss may leave a `.openphoto-...tmp` staging file; interrupted exports are not resumed automatically, and unknown files are not searched/deleted on startup.

Existing project limits apply (one source/kept range, up to two hours / 100 megapixels per input frame, even canvases up to 4096 pixels per edge / 16 megapixels). Each encode/decode pass is limited to six hours; export output is capped at 16 GiB. Preflight space estimates are conservative, not guarantees against a later disk-full, filesystem-size, quota, or unplug failure. Those failures are surfaced before publication. All native output/progress/diagnostic allocations are bounded; measured whole-process memory remains an acceptance task.

## Verification and remaining acceptance

Generated fixtures compare source/export frame timestamps within 1 ms, sample decoded preview/export pixels, check audio gain/mute/start/end timing, and fully decode output. Coverage includes a 90-second VFR/audio source, jittered timestamps, non-frame-grid trims, portrait output, HDR rejection, cancellation, corrupt output after header checks, changed source, insufficient-space simulation, and destination races.

Phase 6 remains in progress until real installed-Windows media/hardware, hour-scale synchronization, drive/disk failures, installer independence from system FFmpeg, and measured performance/memory pass. Browser marker placement is approximate; generated timing tests do not establish frame-accurate user seeking on every codec. HDR tone mapping and waveform UI remain deferred; Phase 7 multi-video editing is not part of this release.
