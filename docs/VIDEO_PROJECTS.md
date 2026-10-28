# Single-video editing projects (0.54.3)

Open a video and choose **New project from selected video**. The original player then uses the project's saved in/out points. Set trim values or use I/O markers; edit crop, quarter-turn rotation, flips, audio gain/mute, and canvas placement. Crop coordinates refer to the displayed, oriented source before additional project rotation. Free crops and centered square/landscape/portrait presets are available. Fit adds black borders; fill crops centrally. The default canvas accounts for source rotation.

Choose **Preview edits** to render the current saved instructions. This closes the original playback view while rendering/playing the derivative; **Back to source** returns to the original. Changing an edit cancels and invalidates the old result. Previews are SDR H.264/AAC, use square pixels, and fit within 960 by 540. Audio uses the first source track as stereo; extra tracks are disclosed. Native processing receives validated arguments and the same resource/cancellation controls as other preview jobs. This is not video export.

## Saving and recovery

Drafts autosave under the local Open-Photo application-data folder in `video-projects.sqlite`, independently of preferences and media caches. Each successful write increments a revision; stale writers are rejected. The queue serializes/coalesces changes and survives player unmounts. Window closure waits for pending saves; failures remain visible with retry or explicit close-without-saving. A failed session can save a recovery project copy or explicitly discard unsaved edits and reload its durable draft. SQLite keeps up to 20 previous documents per project; a UI for browsing those historical documents is not implemented.

**Recent projects** lists the latest 30 drafts for the selected library. Select one after restart to resume the saved state. Undo/redo keeps up to 100 edits during the current session; current edit settings persist across restarts, undo stacks do not. Reset is itself undoable. Organizational photo/AI data is unaffected.

**Save project copy** writes a versioned JSON document to a new filename, using staged publication without overwrite. It contains one asset and one clip with stable IDs, exact source-time tick strings/time bases, edit settings, and canvas settings. It references the original source relative to the selected library and includes full SHA-256 identity; it does not embed video bytes. Keep the source and project copy backed up together. **Open project file** validates and imports a new local draft, preserving asset/clip identities while assigning a fresh project ID to avoid overwriting another draft.

## Source identity and relinking

Creating/opening/rendering verifies the source's full SHA-256 and length without loading the video into memory. Source timestamps and edit instructions remain separate from derived previews. Missing/moved/changed sources cannot produce a trusted preview. Use **Relink original video** to select an identical file inside the current library; byte-different replacements are rejected. To use genuinely changed footage, create a new project. Portable project paths resolve against the current library root, so importing a project copy into a moved library can recover without an absolute old path. Files outside the chosen library and traversal paths are rejected.

## Limits and remaining acceptance

Schema version 1 supports one asset/clip, a minimum 0.04-second kept range, videos up to two hours and 100 megapixels per frame, crops of at least 1% on each axis, audio gain 0–200%, and even output canvases up to 4096 pixels per edge / 16 megapixels. Canvas aspect is bounded to 32:1. Imported documents are limited to 1 MB and unsupported schema/fields are rejected. HDR rendering is unavailable pending tested tone mapping.

Selection is derived from approximate browser playback time, quantized into source ticks. Rendered preview validation allows two average-frame durations with a 100 ms floor for container/audio padding. Tests additionally compare VFR frame timestamps within 1 ms, decoded color regions, and audio gain. Frame-accurate export, export/preview agreement, and long real-media synchronization remain separate acceptance gates.

Still required: installed Windows tests for restart recovery, window close during saves, copied real media, long paths/videos, unavailable drives, disk failures, and hardware behavior. Export, waveform UI, and HDR tone mapping are not delivered in this release.
