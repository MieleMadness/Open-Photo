# Open-Photo feedback roadmap

This roadmap organizes the October 2026 hands-on app feedback by implementation owner and by phase. The goal is to improve the current experience without duplicating work already planned in Phase 7.

## Ownership guide

### Good fits for direct ChatGPT/GitHub implementation
These are localized UI, navigation, styling, and low-risk behavior changes that can be reviewed and pushed directly:

- Standard page-header pattern and hierarchy.
- Remove the top breadcrumb bar and relocate appearance controls to the left navigation.
- Move workspace and folder management out of Photo Library.
- Make Photo Library content-first.
- Folder-management UX, indexing status, and folder stats presentation.
- Map wrapping/gray-area fixes and basic empty/help states.
- Viewer spacing, width, file-summary order, next/previous navigation.
- Video transport controls such as five-second skip buttons.
- Icon treatment for undo/redo and other obvious actions.
- Copy/spacing cleanup such as Dismiss and autosave/export notes.
- Thumbnail filename spacing.
- View controls and incremental gallery presentation improvements.

### Better suited to Codex
These touch larger data models, native rendering/export, long-running media jobs, or coordinated changes across Rust and React. Codex should implement these from a detailed issue/spec while CI and acceptance tests remain mandatory:

- Add multiple export container/codec choices beyond the validated MP4 path.
- Unify single-video editing fully into the multi-asset timeline editor.
- Dashboard architecture if it requires a new persistent home-data aggregation layer.
- Masonry virtualization at large-library scale.
- One search box that merges filename, OCR, semantic-image, semantic-video, people, and tags with ranked results.
- Video semantic search based on sampled frames and/or scene detection.
- Map thumbnail clustering, large-library marker virtualization, and HEIC/MOV GPS extraction.
- External music-provider integration, download/cache/licensing metadata, and attribution workflows.
- Any change to FFmpeg/native rendering graphs, project schema migrations, or export validation.

## Already part of Phase 7

Phase 7 is the multi-asset editing program. Do not create parallel single-video implementations for these features:

- Multiple assets in one project — foundation implemented in 0.56.3.
- Sequential timeline assembly — planned.
- Split clips — planned.
- Trim clips — planned.
- Reorder clips — planned.
- Ripple delete — planned.
- Linked audio — planned.
- Mixed-media normalization — planned.
- Save/relink/recovery for multi-asset projects — partially implemented/further work planned.
- Continuous combined export — planned.
- Start with hard cuts — planned.

The following feedback should extend Phase 7 after the hard-cut timeline is stable:

- Single-video and multi-video should share the same editor UI.
- Clip transitions.
- Text overlays on images and video.
- Still images on the video timeline.
- Music/audio tracks.
- Social-media/Reel presets.
- Multiple export formats if they are supported by the native export engine.

## Proposed execution phases

### Feedback Phase 1 — Navigation and viewer cleanup
Owner: ChatGPT/GitHub

- Standardize the page heading pattern.
- Ensure nothing appears above each page heading.
- Remove the breadcrumb/top bar.
- Move light/dark controls into the left rail.
- Move workspace selector and folder controls to Folders.
- Make Photo Library content-first.
- Make Change Folder route to folder management.
- Add previous/next navigation in file viewer.
- Widen and clean up the detail panel.
- Put file identity/details before video playback and technical video details after playback.
- Change video skip to five seconds.
- Remove Go to time.
- Replace Mark in/out language with saved trim start/end controls.
- Use icons for undo/redo.
- Simplify Dismiss export.
- Improve thumbnail filename bottom spacing.
- Fix map wrapping and gray bars.

### Feedback Phase 2 — Library home and discovery
Owner: ChatGPT for product/UI architecture; Codex for scale-sensitive implementation

- Decide whether Open-Photo opens to an iCloud-style Dashboard.
- If approved, add Dashboard as the default landing page with recent media, memories, library health, storage/index stats, recent video projects, and shortcuts.
- Keep Photo Library focused on media rather than management.
- Add gallery modes: uniform grid first, then justified/masonry if performance remains acceptable.
- Replace Timeline page with a side date scrubber in Photo Library.
- Improve Map with location counts, thumbnail previews, clusters, and useful no-GPS guidance.
- Add folder-level stats.

### Feedback Phase 3 — Unified search
Owner: Codex for backend/index work; ChatGPT for UX

- One top-right search field.
- Rank filename, tags, OCR text, semantic image matches, people, and collections together.
- Add filters for photo/video/date/person/tag/location.
- Add semantic video indexing by sampling representative frames.
- A query such as burger should be able to find a video when a burger appears during the clip, not only when it appears in the poster frame.

### Feedback Phase 4 — Phase 7 timeline completion
Owner: Codex

- Timeline clips.
- Split/trim/reorder/ripple-delete.
- One editor for one clip or many clips.
- Still-image clips.
- Linked audio.
- Combined preview and continuous export.
- Preserve current project migration, relink, recovery, and source-read-only guarantees.

### Feedback Phase 5 — Basic social-content editor
Owner: Codex, after Phase 7 hard-cut timeline is stable

Minimum useful Reel/social feature set:

- Portrait 9:16, square 1:1, landscape 16:9 presets.
- Video and still-image clips.
- Clip trim/split/reorder.
- Hard cuts plus a small transition set: crossfade, dip-to-black, slide.
- Text overlays with font, size, alignment, safe margins, start/end duration.
- Background music track with trim, fade in/out, volume, mute/duck source audio.
- Simple audio waveform.
- Canvas fit/fill and repositioning.
- Basic brightness/contrast/crop/rotation already available where applicable.
- Export presets optimized for social video.
- Project autosave and recovery.

### Feedback Phase 6 — Music library integration
Owner: Codex after provider/license decision

Potential providers must be evaluated for API terms, commercial use, attribution, caching, and redistribution. Do not assume royalty-free means unrestricted.

Preferred architecture:

- Provider search is optional and online.
- Download selected track locally into the project cache.
- Save provider, track ID, artist, license, attribution, and source URL with the project.
- Never hotlink a track during final export.
- Show licensing/attribution before the user adds a track.
- Support importing the user's own local audio regardless of provider integration.

## Product decisions

### Workspaces versus projects
A workspace should remain a library boundary: one logical photo/video library with its folders, collections, people names, tags, and indexes. Projects belong inside a workspace. A video project is not a workspace.

### Map purpose
The map should become a discovery view, not just a Leaflet demonstration. It should answer: Where were my photos/videos taken? What media is near this place? What dates and trips are represented here?

### Video semantic search
Current semantic image search is not enough to guarantee discovery of an object appearing mid-video. Supporting this well requires indexing sampled frames or detected scenes and connecting those embeddings back to video timestamps.

## Current implementation status

Feedback Phase 1 began in 0.56.4.
