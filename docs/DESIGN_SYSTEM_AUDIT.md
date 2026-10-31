# Design-system audit — Phase A

Baseline: 0.56.5. Scope: the current React frontend and its existing rendered-fixture evidence. This is a product UI consistency review, not a claim of native-screen-reader or performance certification. Keep the established visual system; apply corrections as each working area changes.

## Current inconsistencies and incremental response

| Area | Before | After / reason |
| --- | --- | --- |
| Sidebar, headers | Date browsing is distinct, but page heading spacing and compact-label behavior still vary. | One modest page heading, optional short help, primary action beside it; labeled compact navigation with hover/focus tooltips. |
| Media library | Grid virtualization exists; list mode renders the full result set. Search/indexing tools still have separate result experiences. | Preserve the grid; virtualize list results in Phase G and share ranked discovery in Phase E. |
| Folders/workspaces | Folder management works, while video projects are still stored under a source-root key. | Workspaces own projects; folders are source locations, not project identity. Migrate storage in the unified-editor work. |
| Map | One circle per photo, literal marker colors, effect-driven map recreation, JPEG/TIFF-only GPS extraction. | Keep one map instance, clustered discovery, themed markers, explicit counts/empty states; expand safe extraction in Phase H. |
| Collections, Memories, People, Tags | Shared controls exist but section density, empty/help copy, and async feedback differ. People correction remains limited. | Consolidate page/section/status primitives incrementally; retain current features and distinguish unavailable data from no results. |
| Settings | Reusable theme/form/error rules exist, but some cards and diagnostic/status presentations remain local. | Prefer compact sections and shared status/error conventions; keep advanced diagnostics disclosed. |
| Details, photo viewer | Inspection and immersive viewing have separate geometry, with shared modal focus protection. | Preserve this distinction; viewing prioritizes media and inspection exposes metadata/actions. |
| Video player/editor | Single-video editing and the multi-video bin still use separate controllers; there is no editable sequence timeline. | One project controller and editor, with source preview explicitly distinct from assembled preview. |
| Export panel, modals | Live notices can move into the active modal without remounting job owners. | Reuse that mechanism; progress, retry, cancellation, and close protection must remain reachable. |
| Buttons/inputs/selects | Foundation states exist, but newer controls must not invent their own palette or validation behavior. | Use the existing control hierarchy and semantic tokens; keep invalid drafts with field-level explanations. |
| Motion/states | Reduced-motion support and short overlay transitions exist. Timeline gestures are not implemented yet. | Direct manipulation follows the pointer; keyboard actions are immediate. Animate explanations, not routine browsing. |

## Tokens and spacing

`src/tokens.css` remains authoritative: canvas/surface/inset, normal/muted text, accent fill, accent text, on-accent, selected surface, borders, success/warning/error roles, focus, and motion. Extend a semantic role only when a real component needs it. Never reuse accent fill as a text foreground without testing its actual background.

Use the existing **4 / 8 / 12 / 16 / 20 / 24 / 32px** scale. Default controls are 42px; compact desktop controls may be smaller with adequate hit area/spacing. Use existing 8/12/16px radii by role. Prefer a border for ordinary panels and elevation only for floating surfaces. Do not add giant statistics cards, decorative gradients, or hero-sized headings.

## Standard page and type structure

One page title → short optional explanation → task toolbar → working content → contextual status. The working content receives the remaining window area. Each scrolling/sticky region has one named owner.

Use the system UI family. Suggested hierarchy: 28px page title, 20px section title, 16px subsection, 14px body/control, 12px secondary metadata. Long instructions belong in contextual help. Timeline clocks use tabular numerals. Do not hide essential instructions through tiny type.

## Buttons, dialogs, and drawers

Filled primary: the main action. Neutral secondary: related actions. Quiet/icon action: repeated compact commands with an accessible name and tooltip. Destructive project-reference removal must explain that originals remain. Provide default, hover, pressed, focus, disabled, loading, error, and success states where applicable.

Use the existing `Modal` for protected-focus tasks, with initial focus, containment, Escape, and restoration. Use `TaskForm` for recoverable forms and native pickers for files. Keep inspection separate from sustained editing. Live export/save controls must remain inside an active modal's focus boundary.

## Motion and accessibility

Use the current 140/220ms timing tokens for appropriate occasional changes. No timeline selection, scrubbing, or keyboard-command entrance animation. Drag previews may track directly without committing an edit on every pointer movement; cancellation must restore the original order. Provide Move earlier/later buttons and keyboard operation alongside drag.

Keep normal enabled text at least 4.5:1 against its actual surface and meaningful non-text states at least 3:1 where required. Preserve semantic labels, visible focus, native disabled states, field errors, progress announcements, and explicit reduced-motion behavior. Test light/dark, loading, empty, error, missing-source, long-name, and 800 × 580 layouts. Existing contrast tests cover semantic foreground/background pairs; they do not establish full accessibility compliance.

## Consolidation priorities

Shared project/session model; timeline command reducer; selected-clip inspector using existing edit primitives; page heading/toolbar; virtualized media result; status/empty/error block; export preset resolver. Avoid a second editor controller for single-video projects or a universal configurable card.
