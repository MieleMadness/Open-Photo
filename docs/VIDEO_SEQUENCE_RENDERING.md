# Continuous hard-cut rendering

Implemented in 0.56.8. Native sequence checks and frontend behavior checks pass; installed WebView2 playback remains a separate acceptance gate.

## Why staged clips

Opening up to 200 original decoders in one filter graph would make memory use depend on the size of the timeline. Instead, render one clip at a time to job-owned intermediate files, then join those files. Video is encoded once per clip and copied into the final MP4. Intermediate audio stays as 48 kHz stereo floating-point PCM; encode the complete audio track to AAC once, avoiding separate AAC priming delays at every cut. Internal Matroska/PCM is a processing detail, not another user-facing export format.

This trades temporary disk space for bounded decoding memory. Estimate space for both intermediates and final output before starting. Scratch files belong to the job, and cleanup must finish before publishing an export. Existing source files and destinations remain protected by fingerprint checks and staged no-overwrite publication.

## Timing and edits

Keep rational source in/out times in the project. Normalize multi-clip output to the project's 30/60 fps clock. Round cumulative clip boundaries, rather than rounding every duration independently, so rounding does not accumulate across the sequence. Audio intervals use that same frame allocation; preserve source audio offsets/gain and fill silent or muted clips with silence when the project contains audio. A wholly silent/muted project has no audio track.

Reuse the existing source verification, metadata inspection and edit-filter preparation. Hard-cut normalization uses one canvas, pixel format and SDR color contract. Unsupported HDR/wide-gamut conversion remains an explicit error. One-video-clip projects retain the established source-timestamp pipeline. Photo-only projects use the shared project frame clock.

## Required checks

Use very short synthetic sources with distinguishable pictures and sound. Verify order, trims, repeated instances, mixed geometry/frame rates, silence/mute/gain, delayed audio, duration/frame allocation, source fingerprints, and preview/export agreement. Independently validate final video and audio spans, format and geometry, and decode the full staged output before publication. Cancellation, unavailable space, source changes and racing destinations must not publish partial files. Long-form acceptance stays deferred to 1.x.

## Timestamp and Windows path handling

Decoded audio timestamps remain intact until resampling. `aresample=async=1:first_pts=0` reconciles late starts and encoder delay before exact sample allocation; metadata alone must not be used to invent delays. See the [FFmpeg resampler](https://ffmpeg.org/ffmpeg-resampler.html).

Joining Matroska parts uses a private working directory and a generated relative manifest, retaining `safe=1` and the file/pipe protocol restrictions. This avoids FFmpeg resolving relative names incorrectly against Windows verbatim paths. The child process owns this working directory; the app never changes its global working directory. Final copied H.264 packets use `setts` with an explicit 90 kHz clock and `prescale=1`, so packet expressions use the output clock. See [FFmpeg setts](https://ffmpeg.org/ffmpeg-bitstream-filters.html#setts).

## Limits and presets

Only MP4/H.264 SDR and optional AAC stereo are exposed. Explicit HDR/non-BT709 transfer/primaries requiring conversion are rejected. Export size resolves separately from saved edits, caps the requested envelope and never enlarges a smaller canvas. Canvas shape and fit/fill remain editing decisions. The 50-asset/200-clip project bounds and processing/output byte limits still apply; temporary PCM needs additional disk space. Photos are supported by schema 3; text, transitions and extra audio tracks require later extensions in this same editor. Long-form optimization remains in 1.x.

Native acceptance uses sub-second synthetic source clips, including Unicode paths, mixed geometry/frame rates, real encoded audio and full final decode. Cancellation, insufficient-space checks, changed-source and racing-destination tests leave no published partial file or temporary parts. Simulated failures do not replace deferred physical disk-failure acceptance. Browser fixture checks do not replace an installed WebView2 listening/interaction check.


## Still sources — 0.56.21

A photo asset stores oriented width/height and a fingerprint, not video metadata. Its clip timing uses microseconds, bounded to a 600-second virtual interval. Supported files are JPEG/PNG/BMP/TIFF up to 250 MB, 50 megapixels and 20,000 pixels per edge. Animated PNG is rejected explicitly. Existing bounded decoding normalizes EXIF orientation, then alpha is flattened against black into a job-owned PNG. Only one source is decoded at a time. The same clip filter builder handles crop, turns, flips, brightness/contrast and fit/fill; frame allocation and silent audio use the continuous timeline contract. Normalized files are removed with the owned scratch directory. Revalidate originals and fully decode the staged result before safe publication.

The sub-second mixed fixture verifies a video/photo/video sequence, a distinguishable photo frame, silent audio over the photo, preview/export geometry agreement, and a photo-only output with the expected frame count. Migration tests preserve legacy IDs, timing and edits; workspace tests cover save/reopen/history/relink and failed import atomicity. These checks do not replace installed WebView2 acceptance.
