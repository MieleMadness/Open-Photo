# Open-Photo design system

Open-Photo is an offline-first Windows media organizer and basic editor. Its working surfaces prioritize finding, inspecting, and editing local media while preserving originals. Keep the established purple identity and familiar desktop controls.

## Foundations

- `src/tokens.css` owns light/dark colors, spacing, type, radii, control height, and motion tokens. `src/styles.css` consumes those roles. Add tokens there rather than adding release-specific theme overrides.
- Accent fill, on-accent text, accent text, selected surface, and semantic success text are different roles. Check actual foreground/background pairs in both themes: enabled normal text needs at least 4.5:1 contrast. Disabled controls are visually distinct but are not used to justify low-contrast enabled controls.
- Use the system UI font and a restrained fixed hierarchy. Standard controls are 42px; compact toolbar controls may be smaller with adequate hit areas, spacing, and labels. Essential instructions should not use tiny display text.
- Spacing uses the shared 4/8/12/16/20/24/32px scale. Group related actions; avoid additional cards or shadows merely to separate every control.

## Surface ownership

- The app frame reserves room for save/export/update notices. Notices must not cover focused library or editor controls.
- Library content uses the available window height. Search, media type, sorting, and view selection remain immediately available. Tagging is contextual to selection; maintenance and additional search/indexing actions live in the library-tools disclosure.
- The media grid owns its scroll position. Resize observation updates column and row geometry. Caption height and vertical row gap are supplied alongside virtualization calculations; do not add caption rows without updating that contract.
- Generic toolbars are not sticky. Any future sticky behavior needs a named owner and a verified offset; do not solve collisions with additional z-indexes.
- Inspector, full-screen viewer, and video editor are explicit layout variants. Full-screen viewing concentrates on media. The video editor keeps a preview stage beside task-grouped controls on wide windows and stacks them at smaller widths.
- Library date navigation is **By date**. Reserve **timeline** inside video editing for clip positions and sequence time.

## Interaction and states

- `Modal` in `src/Ui.tsx` uses native dialog modality, visible initial focus, a keyboard boundary, Escape/backdrop dismissal, and focus restoration. Reuse it for protected-focus tasks; do not implement a visual backdrop and ARIA alone.
- `TaskForm` provides labeled fields, native range/required constraints, whitespace validation, preserved drafts, and Cancel. Continue using native file pickers for opening/saving files.
- Invalid video numeric drafts remain visible with an explanation. They must not silently revert or export a stale saved value; correct them before export.
- Show original-source versus edited-preview mode explicitly. Autosave state, save errors, export progress, and retry/cancellation remain understandable without relying on color alone.
- `ErrorNotice` separates the user-facing problem/recovery from optional diagnostic detail. Startup errors use the same typography and controls as the rest of the app.
- Keyboard navigation, selection, scrubbing, and repetitive editing feedback should feel immediate. Use short, purposeful overlay transitions; reduced-motion rules target the components instead of globally replacing every duration.

## Verification

Check 1220 × 780 and 800 × 580, light/dark, empty and populated libraries, long filenames, ratings, multiple thumbnail sizes, keyboard-only dialogs, invalid forms, missing sources, and save/export recovery. Use short media fixtures; long-form work is deferred to 1.x. Native playback/export and OS accessibility checks remain distinct from browser fixtures.

The audit fixes do not implement Phase 7 clip sequencing or combined export. Keep those capability claims accurate until they ship.

## Shared editing workflow — 0.56.7

The workspace selector stays at the top left; only the navigation list scrolls on smaller windows. There is no Video Projects navigation tab. The Library opens one workspace-owned editor for one or many videos. The media bin chooses original sources, the timeline owns clip selection/placement, the selected-clip inspector owns edit controls, and the output section owns canvas and rendering actions. Keep source versus edited output explicit.

In 0.56.8 the same stage shows one compiled continuous preview, with its playhead linked to the timeline. Switching back to a source releases the old decoder. Canvas shape/fit remain project edits; export resolution is a separate choice. Numeric source marks retain precision while joined timeline positions follow the project frame clock. Normalize clip widths against total duration, including sub-second edits. Photos and later social extensions belong in this same editor.

## Unified discovery — 0.56.11

Keep one Library search field. Metadata/cached evidence updates while typing; Search/Enter explicitly adds image-content inference. Use the existing controls/tokens and a compact status/help disclosure, with visible strongest-match text in existing tile/list metadata and full reasons in File Details. Relevance is a real sort choice; clearing a search restores browsing order. Preserve current matches while extending a query, disclose partial providers, and cancel stale work without disturbing the newest request. Do not add separate OCR/semantic result pages or decorative search cards.

## Compact navigation and section labels — 0.56.12

The user explicitly prefers green eyebrow labels above section headings. Keep them through future design cleanup; this product preference overrides generic guidance against eyebrows. `PageHeading` owns the structure and uses the contrast-tested success-text role in both themes.

`AppSidebar` owns explicit, saved menu width rather than breakpoint-driven collapsing. Expanded width is 250px, icon rail 72px. Navigation uses 42px rows, 4px gaps and start alignment; extra height stays below rows. Only the navigation list scrolls, keeping workspace selection and the width toggle reachable. Do not animate layout width during frequent media browsing. Collapsed controls retain accessible names, titles and visible focus. Native workspace options remain readable.

File identity belongs in the summary below the File Details toolbar. Keep Edit and Fullscreen / Restore labels short; group browsing and close controls alongside them. Reuse the modal's focus boundary. Map containers observe size changes without recreating the map or resetting its view.

## Desktop editor frame — 0.56.13

The supplied Clipchamp composition and approved plan guide this editor: compact project bar, media left, preview center, clip inspector right, persistent timeline bottom. `src/editor.css` owns editor geometry and shared timeline primitives. The editor fills the available height; the app page must not scroll the timeline away. Media, inspector and timeline own their scroll independently. On narrow widths, side panels become dismissible overlays while panel toggles remain in the header. The application sidebar's saved preference is respected.

Keep one `SequenceSession` and one output controller. `SequenceOutput` exposes render slots so the shell can position preview/export controls without duplicating native jobs or state. `SequenceClipInspector` and `SequenceTimeline` call the same timeline command helper. Hiding a panel does not unmount its fields or discard drafts. Output validation reveals the inspector and native disclosures before focusing an invalid field. Source versus compiled project preview stays explicit.

Use immediate panel/selection feedback and no animated editor width or timeline scrubbing. Controls have accessible names and tooltips. The Project disclosure supports Escape, outside click and keyboard focus departure. Poster work is lazy and bounded; failed thumbnails never block editing or relinking. Future filmstrips and audio lanes must share the same timing and viewport rather than introducing another editor.

## Timeline navigation — 0.56.14

Use a native range for ruler seeking so pointer and keyboard behavior stay shared. Tick labels are decorative, adaptive and bounded to 201 entries. Reuse bounded source posters for clip identity; never imply these are sampled filmstrips. Audio bands describe linked/muted/silent source audio without invented waveforms. Drag insertion maps a before/after boundary into the final order after removing the dragged clip; no-op drops must not enter history.
