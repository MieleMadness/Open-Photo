# Video editing roadmap

Status: Phase 6 is in progress. Version 0.54.2 adds playback/seeking controls, a filmstrip, temporary rational-time preview marks, and cancellable compatible SDR previews, building on 0.54.1 inspection. Saved editing projects and video export remain unimplemented. Phase 6.2 still needs packaged-Windows acceptance with real footage and long/VFR sources; preview/export agreement will be checked when export exists. See [VIDEO_ENGINE.md](VIDEO_ENGINE.md).

## Phase 6 — Single-source video editing

Goal: open one video, make basic nondestructive edits, save the project, and export a new video reliably. Keep one source asset per project in this phase; the project schema should already support multiple clip instances for Phase 7.

### 6.1 — Media engine and capability discovery

- Add Rust `video_probe`, `video_jobs`, `video_project`, and `video_export` modules, with a shared media capability result consumed by the UI.
- Use a pinned FFmpeg/ffprobe distribution and document its included codecs and distribution requirements. Probe duration, streams, codec, dimensions, sample aspect ratio, rotation, frame-rate/time-base information, audio layout, and color/HDR characteristics.
- Launch processes with argument arrays, fixed executable paths, validated local assets, bounded diagnostics, and timeouts. Do not compose shell commands from filenames or permit arbitrary network input URLs.
- Begin with MP4/MOV inputs supported by the packaged decoder set and one validated MP4 export preset. Offer clear errors for unsupported streams. Identify HDR and either implement tested tone mapping or explicitly reject HDR export in the first release.

Acceptance: corrupt files, Unicode/space-containing filenames, rotated phone footage, silent clips, variable-frame-rate recordings, and unsupported codecs return accurate capability results without changing originals.

### 6.2 — Playback, seeking, and proxies

- Stream local assets using bounded range reads; restrict access to registered project assets rather than exposing arbitrary filesystem paths.
- Create cancellable proxy jobs for media WebView2 cannot play smoothly. Track source/proxy timing and orientation so edits map back to the source.
- Add play/pause, scrubber, current time, duration, in/out markers, keyboard seeking, and a filmstrip. Add a basic waveform if it fits this milestone.
- Use exact source timestamps/rational time bases internally; avoid making floating-point seconds the authoritative edit representation. Variable-frame-rate sources require explicit preview/export timing tests.

Acceptance: a clip larger than 48 MB seeks without loading the entire source into JavaScript; switching sources cancels or invalidates old work; proxy preview and source export agree on selected boundaries.

### 6.3 — Basic edits and project persistence

- Trim beginning/end; rotate in quarter turns; flip; crop with free and common aspect presets; fit/fill output canvas; mute and volume. Include simple brightness/contrast only when preview/export agreement is tested.
- Save a versioned project with asset ID, source fingerprint, relative path plus relink information, source time range, transforms, audio gain, and output settings. Never store edits only as UI state.
- Provide undo/redo, reset, autosave, recovery after restart, and explicit relink when a source moves. Keep unsaved-edit behavior clear when closing or switching media.
- Separate source metadata, editable project state, and derived caches. A changed source invalidates derived caches and requires project revalidation.

Acceptance: reopen a saved project and obtain the same edit; undo restores prior values; moving a source prompts relink; original hashes stay unchanged. One source with one kept range is enough for the first milestone. Interior cuts and repeated clip instances can follow in Phase 7.

### 6.4 — Reliable export

- Export accurate trims through decode/re-encode by default. A later stream-copy fast path must advertise keyframe limitations and only activate for eligible edits.
- Start with one tested SDR MP4 preset, original-size or 1080p options, aspect-preserving dimensions, and intentional audio mapping. Do not silently discard extra streams or HDR metadata without explaining the chosen output policy.
- Expose progress, elapsed time, cancel, and useful errors. Write to a unique temporary output; validate the result; publish to the destination with no-overwrite semantics. Preserve completed user files and clean only job-owned temporary files.
- Add one-job-at-a-time scheduling initially. Detect insufficient disk space where feasible and handle write failures without leaving a final-looking broken export.

Acceptance: exported duration and trim boundaries match the project within a documented decoded-frame tolerance; A/V synchronization passes short and long fixtures; cancellation terminates the child process; source and existing destination files remain unchanged; installer contains the required engine and works without a separately installed FFmpeg.

### Phase 6 completion gate

All current CI gates plus real packaged-Windows tests pass. Cover silent/audio clips, portrait rotation, variable frame rates, codec fallback, long paths, source removal, cancellation, disk-full failure, reopen/relink, and output frame checks. Record startup, seek latency, export speed, and peak memory on a named reference PC. Choose numeric performance budgets after the initial baseline rather than promise unmeasured responsiveness.

Deferred: multiple source assets, transitions, titles, background music, overlays, speed ramps, keyframes, auto captions, and AI video effects.

## Phase 7 — Basic multi-video timeline

Goal: assemble several videos into one coherent output with familiar drag, split, trim, reorder, and export interactions. The first delivery needs a single sequential video track with linked source audio, not a full compositing editor.

### 7.1 — Multi-asset projects and migration

- Migrate Phase 6 projects into a multi-asset schema without losing edits.
- Each clip instance has an immutable ID, asset reference, source in/out, timeline placement or sequence order, transforms, and audio settings. One asset can appear repeatedly.
- Keep source time and timeline time distinct. Define one project canvas, frame rate, and audio output format. Retain source timing metadata for conversion.
- Add a media bin, multi-file import, missing-source relink, and duplicate-import detection.

Acceptance: migrated single-video projects export identically; repeated use of one asset survives save/reopen; source relink updates all relevant instances intentionally.

### 7.2 — Sequential timeline editing

- Drag clips into a sequence; reorder; trim both ends; split at playhead; delete with ripple; duplicate; zoom timeline; snap to boundaries. Preserve linked audio during edits.
- Define MVP deletion as ripple deletion so the single-track model cannot accidentally create unexplained gaps. Make edit commands transactional and undoable.
- Use a deterministic timeline reducer/command model with unit tests for split/trim boundaries, ordering, and undo/redo.

Acceptance: adjacent cuts do not introduce extra black frames or audio gaps; splitting then rejoining an untouched clip retains timing; mixed clip lengths and repeated edits save/reopen correctly.

### 7.3 — Mixed-media preview and export

- Normalize frame rates, geometry, rotation, pixel format, and audio layout for the chosen project output. Show fit/fill and background color for portrait/landscape mixing.
- Generate preview segments/proxies from the same timeline representation used by export. A chain of independent HTML video players is insufficient to guarantee continuous timeline playback.
- Use an export graph built from validated clip records. Supply silence where a clip lacks audio, reset timestamps appropriately, and verify cumulative A/V synchronization.
- Begin with hard cuts. Add a basic title card, audio fade, or one background-music track only after sequential playback/export is reliable.

Acceptance: mixed portrait/landscape, silent/audio, and variable/constant-frame-rate clips export in order with consistent canvas and duration. Test 2, 10, and 50 clips, boundary frames, long projects, cancellation, and memory. Decode and inspect output rather than only checking process exit status.

### 7.4 — Usability and release gate

- Add keyboard shortcuts, clear selected-clip state, timeline navigation, export summary, autosave/recovery, and relink across multiple assets.
- Release when real Windows users can assemble a basic video without consulting developer instructions, and project recovery/export are dependable.
- Keep transitions, arbitrary overlapping video tracks, masks, animation/keyframes, speed ramps, advanced audio mixing, and a plugin marketplace outside the initial scope.

## Future features ranked by practical value

1. Durable metadata backup/export, project recovery, and relink across photo and video workflows.
2. Complete tag editing: rename/remove with preview, undo, and preservation of unknown XMP fields.
3. Unified cancellable jobs panel for scans, AI, proxies, and exports.
4. Proxy/cache controls with disk usage and safe cleanup.
5. Cross-root duplicate review with persistent hashes and conservative identity handling.
6. Reviewed People assignments, merge/split correction, and stable identities.
7. Video poster frames, waveforms, and reusable export presets.
8. Basic titles and a single music track after Phase 7 hard cuts are stable.
9. Immich integration as a separately scoped milestone with an actual tested server version and explicit mapping review.
