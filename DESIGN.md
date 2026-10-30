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

The workspace selector stays at the top left; only the navigation list scrolls on smaller windows. There is no Video Projects navigation tab. The Library opens one workspace-owned editor for one or many videos. The media bin chooses original sources, the timeline owns clip selection/placement, the selected-clip inspector owns edit controls, and the output section owns canvas and rendering actions. Keep source versus edited output explicit. One-clip native output is retained; continuous output is gated until validated.
