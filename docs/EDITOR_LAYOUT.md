# Desktop editor layout

## 0.56.13: implemented frame

- Top: Back to Library, project name, saved status, Project menu, panel toggles and Export.
- Left: import, filename search, lazy thumbnails and selected-source options.
- Center: original source or one compiled continuous project preview, with clear mode labels.
- Right: Clip and Canvas & export tabs separate selected-clip controls from project output settings (0.56.17). A header Canvas button opens output settings directly.
- Bottom: existing proportional hard-cut timeline, playhead, zoom, undo/redo and Add clip.

The supplied Clipchamp screenshot guides panel placement. Open-Photo retains its own light/dark tokens, purple identity, green section labels and offline source-safety behavior. Workspace selection remains at the top left and no Video Projects navigation tab is added.

## Architecture

The project session remains the sole editable model. The timeline and inspector invoke shared commands; neither stores a second copy of clip edits. Output control slots place buttons/settings in different regions while keeping one render/export owner and its cancellation/revision checks. One stage owns playback; source and edited output are never two simultaneous audio players.

Panel hiding preserves fields and drafts. Export validation opens hidden controls and focuses the invalid field before output proceeds. Missing source relinking and history/recovery keep the existing verified workspace commands. Posters are requested only near visibility, with at most two native jobs and a 100-entry identity cache. Unmounting skips queued work and discards stale UI results.

## Follow-up patches

1. Timeline filmstrips. Trim handles shipped in 0.56.15 and the adjustable preview/timeline divider in 0.56.16. The adaptive time ruler, clip posters and before/after insertion markers shipped in 0.56.14. Retain numeric trim and move buttons as alternatives.
2. Photos as duration-based clips in the same project, with verified source decoding and joined preview/export. Reconcile the preserved mixed-media work against the current branch before integration.
3. Short mixed-media acceptance: two videos plus one photo, trim/split/reorder, save/reopen, joined audio/video export and unchanged originals.
4. Local audio import and waveforms, then text and a small tested transition set under the existing roadmap gates.

## Verification and limits

Use normal/wide/minimum windows, both themes, expanded/collapsed app navigation, hidden panels, long names, empty media search, missing sources, invalid drafts, source compatibility fallback, preview cancellation and revision-safe export. Browser fixtures complement native/installed checks. Use short fixtures; long-form acceptance remains deferred. The layout patch does not add photo rendering, new transitions, instant live recomposition or an online music catalog.


## 0.56.18 editor interaction decisions

- Source trimming is performed only from the timeline clip handles. Do not reintroduce duplicate trim controls in the preview/player area.
- The preview stage uses the browser/native video controls for playback. Open-Photo-specific Play, skip-five-seconds and duplicate scrubber controls are intentionally hidden in the editor.
- Compatibility playback remains available as a small action near the preview rather than a separate details/playback-options section.
- The horizontal divider directly controls preview versus timeline space and must not reset during an active pointer drag.
- Timeline zoom uses explicit Zoom out / Fit / Zoom in controls.
- Timeline clips should visually preview their video content. 0.56.18 uses repeated cached posters as a safe presentation layer; replace these with bounded sampled frames when a native filmstrip provider is available.
- Crop belongs to the selected clip inspector and remains nondestructive.
