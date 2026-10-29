# 0.56.5 design audit coverage

This update implements the recommendations from the 29 October 2026 audit of 0.56.4. The app's identity, source protection, project formats, and export behavior remain intact.

| Finding | Implementation |
| --- | --- |
| F01: media below the initial viewport | Replace statistics cards with a compact summary, disclose maintenance tools, show tagging after selection, and give media the remaining height. |
| F02: overlapping sticky toolbars | Remove sticky positioning from the generic toolbar class. |
| F03: constrained full-screen viewer | Give the viewer a specific full-window variant, hide editing/metadata controls during viewing, and reset the panel scroll on layout changes. |
| F04: dialog keyboard focus | Reuse a native modal dialog with focus containment, visible initial focus, Escape, and restoration to the invoker or current navigation fallback. |
| F05: theme contrast | Add accent-text, on-accent, and success-text roles instead of pairing white text with the lighter dark-theme purple. |
| F06: clipped captions | Reserve a caption track; use shared caption/gap values in virtual row sizing and respond to window-width changes. |
| F07: compact navigation | Hide brand text explicitly, use distinct destination icons, and supply labels/tooltips. |
| F08: sustained editing in a narrow drawer | Open a wide video-editor variant, separate preview/control scrolling, and group Trim and transform, Audio, Adjustments, and Export. |
| F09: prompts and silent invalid input | Replace workspace/collection/rule/person prompt chains with labeled forms. Keep invalid video numeric drafts and block export until corrected. |
| F10: misleading labels and safety copy | Use mixed-media vocabulary, distinguish library versus selected size, rename date navigation, and describe managed XMP merges/separate exports accurately. |
| F11: design-system drift | Centralize light/dark foundation tokens, introduce shared dialog/form/error primitives, remove release-specific CSS section naming, and document ownership in DESIGN.md. |
| F12: repeated video-project hierarchy | Keep one page heading and collapse create/switch actions after loading a project, bringing the bin and source preview upward. |
| F13: inconsistent recovery | Style startup recovery, provide filter reset and reusable diagnostic disclosure/retry, and dock persistent notices outside the working surface. |

## Verification boundary

Frontend tests exercise saved-workspace lifecycle, focus boundaries/restoration, form draft validation, invalid numeric export prevention, video edits, durable projects, cancellation, updates, and recovery. TypeScript checking and production bundling are required before committing.

Local browser checks use the actual React frontend with an isolated in-memory Tauri bridge, fictional filenames, generated preview images, a 12-second video project, and a two-video project. They check layout, caption bounds at 110/180/300px thumbnail settings, normal/minimum window size, modal focus/restoration, cancellation, clear filters, explicit reduced motion, and uncaught page errors. They do not prove native playback, file access, AI inference, or real exports. No original media is used or changed by these fixtures.

The existing GitHub workflow must pass frontend checks, Rust checks, and the Windows installer build after pushing. No long-form media acceptance is added. Physical-drive failure remains a separate pre-v1.0 check; further long-form work remains deferred to 1.x.
