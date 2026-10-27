# Local video engine (Phase 6.1)

Open-Photo 0.54.1 bundles FFmpeg and ffprobe for read-only video inspection on Windows x64. No system-wide FFmpeg installation or runtime download is required. Existing playback and the Windows player fallback remain available.

## Reproducible preparation

Run `npm run prepare:video`. The script downloads a fixed BtbN LGPL shared build, verifies the archive SHA-256, and copies ffmpeg, ffprobe, their DLLs, license texts, and an engine manifest into the generated `src-tauri/resources/video-engine/` directory. These generated binaries are ignored by Git. The installer bundles that directory under `video-engine/`.

- Engine: `n8.1.3-24-g92611900bb`
- Distribution release: `autobuild-2026-10-27-18-38`
- Archive: `ffmpeg-n8.1.3-24-g92611900bb-win64-lgpl-shared-8.1.zip`
- SHA-256: `6c6167da57decb941a0466b60d95f7f51e4edd6b2d79c24dc4860c1c15701292`
- [Pinned distribution](https://github.com/BtbN/FFmpeg-Builds/releases/tag/autobuild-2026-10-27-18-38)
- [Upstream build scripts and component sources](https://github.com/BtbN/FFmpeg-Builds)
- [FFmpeg source](https://github.com/FFmpeg/FFmpeg)
- [FFmpeg licensing information](https://ffmpeg.org/legal.html)

The unmodified package's LICENSE.txt and the engine's own license output are included alongside the executables. This is the LGPL shared build, not the GPL build. Preserve these notices and pinned source/build references when redistributing the installer. Runtime libraries remain separate DLLs.

## Inspection behavior

`inspect_video` validates a canonical library media path and launches the bundled executable directly with argument arrays, with no shell or PATH lookup. It permits only the local file/pipe protocols and MOV/MP4, Matroska/WebM, and AVI demuxers. It does not accept streaming playlists or network URLs.

Inspection runs off the UI thread, one job at a time. Selecting another clip supersedes pending work and terminates an obsolete running inspection. Both output pipes are drained concurrently, output allocations are capped, and the process is killed and reaped after a 30-second timeout. Probe/allocation limits bound metadata analysis; they do not impose the old 48 MB inline-playback limit.

The result preserves exact average/nominal frame-rate fractions and time bases. Different average and nominal rates are reported as possible variable timing, not proof of constant or variable frame rate. HDR is detected from PQ/HLG transfer characteristics and HDR/Dolby Vision metadata; unspecified color characteristics are not claimed to be confirmed SDR. Attached cover art is excluded when selecting the video stream. Missing duration or dimensions produce an explicit failure.

The separate `video_engine_status` command reports the pinned engine version and executable decoder/encoder inventories for subsequent editing milestones. Inventory availability is not a guarantee that every codec, driver, or damaged input can decode successfully.

## Build and tests

Run `scripts/check.ps1 -Package` from PowerShell after installing Rust, Node, and the Microsoft C++ workload. The script locates the C++ tools when needed and keeps compiler scratch files and the native AI dependency cache inside writable build directories. This also avoids cross-directory archive-replacement restrictions in restricted local build sessions.

The frontend checks cover displayed properties, stale inspection responses, and failure/retry UI. Rust tests cover JSON parsing and generated real MP4/MOV files, audio/silent clips, Unicode/space-containing paths, rotation, HDR flags, variable frame timing, corrupt media, rejected playlists, timeout/cancellation, and source-byte preservation. Fixtures are generated with the exact bundled engine and removed after the tests.

## Not yet implemented

Single-video trim/crop/rotate/audio edits, proxy generation, saved editing projects, undo/redo, and video export remain Phase 6 work. Reading a video's metadata does not establish that it can be edited or exported successfully. HDR export stays unavailable until a tested tone-mapping workflow is implemented.
